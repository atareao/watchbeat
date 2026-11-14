use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::FutureExt;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

use async_trait::async_trait;

use crate::breaker::{self, BreakerConfig, BreakerDecision, StormBreaker};
use crate::checker;
use crate::checker::Checker;
use crate::db::Database;
use crate::jitter;
use crate::models::{CheckResult, Monitor, Notifier};
use crate::netcheck::NetworkHealthHandle;
use crate::notifier;
use crate::template::{self, TemplateContext};

use crate::attribution;

// ───── Commands ─────

/// The last *observable* status the scheduler saw for a monitor.
///
/// Deliberately distinct from the notification state (`was_up`): an
/// unattributable failure is [`ObservedStatus::Indeterminate`], which is a real
/// change worth persisting and emitting **once**, yet must NOT flip `was_up`
/// (D7). Keeping the two apart is what stops a sustained egress outage from
/// writing a row and emitting an SSE event on every single check (M5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservedStatus {
    /// No check has run yet for this monitor task.
    Unknown,
    Up,
    Down,
    Indeterminate,
}

impl ObservedStatus {
    /// Classify a persisted status string. `is_up` is the notification-state
    /// reading of the very same status (`up`/`warning`).
    fn from_status(status: &str, is_up: bool) -> Self {
        if status == attribution::INDETERMINATE {
            Self::Indeterminate
        } else if is_up {
            Self::Up
        } else {
            Self::Down
        }
    }
}

#[derive(Debug)]
pub enum SchedulerCommand {
    Spawn(Monitor),
    Update(Monitor),
    Remove(String),
    ReloadNotifiers,
    /// Re-read the storm-breaker configuration from the settings (Fase 5, D6).
    ReloadBreaker,
    Shutdown,
}

// ───── Manager ─────

#[derive(Clone)]
pub struct SchedulerManager {
    pub active_tasks: Arc<AtomicU64>,
    pub last_check_at: Arc<AtomicI64>,
    tx: mpsc::Sender<SchedulerCommand>,
    /// The one storm breaker shared with every monitor task. Owned by the
    /// manager so read-only consumers (e.g. `GET /api/breaker`) observe the
    /// **real** state the scheduler is acting on.
    breaker: Arc<Mutex<StormBreaker>>,
}

impl SchedulerManager {
    /// Start the scheduler. Loads existing monitors and begins monitoring.
    ///
    /// `health` is the publication handle of the `netcheck` canary; it is read
    /// (never awaited) on every check to attribute failures.
    pub async fn spawn(
        db: Database,
        event_tx: broadcast::Sender<String>,
        health: NetworkHealthHandle,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let active_tasks = Arc::new(AtomicU64::new(0));
        let last_check_at = Arc::new(AtomicI64::new(0));

        // Fase 5 (D6–D8): the breaker is created here — the only place with both
        // the `db` (settings source) and the chance to share it — and handed to
        // the manager loop, which clones it into every monitor task.
        let breaker: Arc<Mutex<StormBreaker>> = Arc::new(Mutex::new(StormBreaker::new(
            BreakerConfig::load(&db).await,
        )));

        let mgr = Self {
            active_tasks: active_tasks.clone(),
            last_check_at: last_check_at.clone(),
            tx: tx.clone(),
            breaker: breaker.clone(),
        };

        tokio::spawn(async move {
            manager_loop(
                rx,
                db,
                event_tx,
                active_tasks,
                last_check_at,
                health,
                breaker,
            )
            .await;
        });

        mgr
    }

    /// The storm breaker shared with the running monitor tasks.
    ///
    /// Read-only observers take the lock only for the synchronous snapshot
    /// accessors, so they never delay a check.
    pub fn breaker(&self) -> Arc<Mutex<StormBreaker>> {
        self.breaker.clone()
    }

    pub async fn send(&self, cmd: SchedulerCommand) {
        let _ = self.tx.send(cmd).await;
    }

    pub async fn spawn_monitor(&self, monitor: &Monitor) {
        self.send(SchedulerCommand::Spawn(monitor.clone())).await;
    }

    pub async fn update_monitor(&self, monitor: &Monitor) {
        self.send(SchedulerCommand::Update(monitor.clone())).await;
    }

    pub async fn remove_monitor(&self, id: &str) {
        self.send(SchedulerCommand::Remove(id.to_string())).await;
    }

    pub async fn reload_notifiers(&self) {
        self.send(SchedulerCommand::ReloadNotifiers).await;
    }

    /// Re-read the storm-breaker configuration and apply it in place (Fase 5).
    pub async fn reload_breaker(&self) {
        self.send(SchedulerCommand::ReloadBreaker).await;
    }
}

// ──── Background manager loop ─────

async fn manager_loop(
    mut rx: mpsc::Receiver<SchedulerCommand>,
    db: Database,
    event_tx: broadcast::Sender<String>,
    active_tasks: Arc<AtomicU64>,
    last_check_at: Arc<AtomicI64>,
    health: NetworkHealthHandle,
    // Fase 5 (D6–D8): the breaker created by [`SchedulerManager::spawn`], shared
    // by every monitor task below. The threshold is a percentage of the *enabled*
    // monitors, so the count is tracked next to it and refreshed after every
    // command that changes the running task set.
    breaker: Arc<Mutex<StormBreaker>>,
) {
    let notifier_cache: Arc<RwLock<HashMap<String, Notifier>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Load notifiers into cache
    if let Ok(notifiers) = db.list_notifiers().await {
        let mut cache = notifier_cache.write().await;
        for n in notifiers {
            cache.insert(n.id.clone(), n);
        }
    }

    // Fase 5 (D6–D8): one breaker shared by every monitor task (created by the
    // manager so it can also be observed from outside via [`SchedulerManager::breaker`]).
    let enabled_monitors = Arc::new(AtomicU64::new(0));

    let mut task_map: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

    // Spawn tasks for existing enabled monitors
    if let Ok(monitors) = db.list_monitors().await {
        for m in monitors {
            if m.enabled && m.monitor_type != "heartbeat" {
                let id = m.id.clone();
                let handle = monitor_task(
                    db.clone(),
                    m,
                    notifier_cache.clone(),
                    breaker.clone(),
                    enabled_monitors.clone(),
                    event_tx.clone(),
                    last_check_at.clone(),
                    health.clone(),
                );
                task_map.insert(id, handle);
            }
        }
    }

    active_tasks.store(task_map.len() as u64, Ordering::Relaxed);
    enabled_monitors.store(task_map.len() as u64, Ordering::Relaxed);

    // Command loop
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SchedulerCommand::Spawn(monitor) | SchedulerCommand::Update(monitor) => {
                // Cancel existing task for this monitor
                if let Some(handle) = task_map.remove(&monitor.id) {
                    handle.abort();
                }
                // Start new task if enabled and not heartbeat
                if monitor.enabled && monitor.monitor_type != "heartbeat" {
                    let id = monitor.id.clone();
                    let handle = monitor_task(
                        db.clone(),
                        monitor,
                        notifier_cache.clone(),
                        breaker.clone(),
                        enabled_monitors.clone(),
                        event_tx.clone(),
                        last_check_at.clone(),
                        health.clone(),
                    );
                    task_map.insert(id, handle);
                }
            }
            SchedulerCommand::Remove(id) => {
                if let Some(handle) = task_map.remove(&id) {
                    handle.abort();
                }
            }
            SchedulerCommand::ReloadNotifiers => {
                if let Ok(notifiers) = db.list_notifiers().await {
                    let mut cache = notifier_cache.write().await;
                    cache.clear();
                    for n in notifiers {
                        cache.insert(n.id.clone(), n);
                    }
                }
            }
            SchedulerCommand::ReloadBreaker => {
                // Re-read the settings and reconfigure in place (D6). The lock is
                // held only for the synchronous `configure` call.
                let cfg = BreakerConfig::load(&db).await;
                breaker.lock().await.configure(cfg);
            }
            SchedulerCommand::Shutdown => {
                for (_, handle) in task_map.drain() {
                    handle.abort();
                }
                break;
            }
        }
        active_tasks.store(task_map.len() as u64, Ordering::Relaxed);
        enabled_monitors.store(task_map.len() as u64, Ordering::Relaxed);
    }
}

// ──── Per-monitor task (with panic recovery) ─────

/// Spawns the per-monitor task with panic recovery.
///
/// Same wiring as [`monitor_task_inner`], plus the shared handles (notifier
/// cache, storm breaker, enabled-monitor count) the manager loop keeps per task.
#[allow(clippy::too_many_arguments)]
fn monitor_task(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    breaker: Arc<Mutex<StormBreaker>>,
    enabled_monitors: Arc<AtomicU64>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<AtomicI64>,
    health: NetworkHealthHandle,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_secs = monitor.interval_seconds.max(1) as u64;
        loop {
            let result = AssertUnwindSafe(monitor_task_inner(
                db.clone(),
                monitor.clone(),
                notifier_cache.clone(),
                breaker.clone(),
                enabled_monitors.clone(),
                event_tx.clone(),
                last_check_at.clone(),
                health.clone(),
                interval_secs,
            ))
            .catch_unwind()
            .await;

            match result {
                Ok(()) => {
                    // Aborted via JoinHandle — exit
                    break;
                }
                Err(panic_err) => {
                    let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_err.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    tracing::error!(
                        "Scheduler: panic in monitor task for '{}': {}. Restarting in 30s",
                        monitor.name,
                        msg
                    );
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn monitor_task_inner(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    breaker: Arc<Mutex<StormBreaker>>,
    enabled_monitors: Arc<AtomicU64>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<AtomicI64>,
    health: NetworkHealthHandle,
    interval_secs: u64,
) {
    // Cache checker once (reused across all checks for this monitor)
    let checker = checker::checker_for(&monitor);

    // Cache notifier IDs (avoids DB query on every status change)
    let notifier_ids = db
        .get_monitor_notifier_ids(&monitor.id)
        .await
        .unwrap_or_default();

    let mut cycle = MonitorCycle {
        db: &db,
        monitor: &monitor,
        notifier_cache: &notifier_cache,
        breaker: &breaker,
        enabled_monitors: &enabled_monitors,
        event_tx: &event_tx,
        last_check_at: &last_check_at,
        checker: checker.as_deref(),
        health: &health,
        notifier_ids: &notifier_ids,
        interval_secs,
        // Track previous status in memory (avoids get_latest_check query per check)
        was_up: true,
        // Track last DB write timestamp for time-based sampling:
        // write at least once per interval_secs to maintain latency data
        last_write_at: 0,
        observed: ObservedStatus::Unknown,
    };

    // Fase 4 (D1–D3): the cycle is jittered and sequential. The RNG is injectable
    // — the jitter itself is unit-tested — so here we only pick the production
    // source of randomness. `StdRng` (seeded from the OS) rather than `rand::rng()`
    // because the monitor task runs on the multithreaded runtime, so it must be
    // `Send`.
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    jitter::jittered_cycle(
        Duration::from_secs(interval_secs),
        &mut rng,
        &mut cycle,
        tokio::time::sleep,
    )
    .await;
}

/// The production cycle of a monitor task: one check plus the in-memory
/// bookkeeping the scheduler keeps between checks (Fase 4, D3).
struct MonitorCycle<'a> {
    db: &'a Database,
    monitor: &'a Monitor,
    notifier_cache: &'a Arc<RwLock<HashMap<String, Notifier>>>,
    /// Storm breaker shared by every monitor task (Fase 5, D8).
    breaker: &'a Arc<Mutex<StormBreaker>>,
    /// Number of enabled monitors, kept up to date by the manager loop.
    enabled_monitors: &'a Arc<AtomicU64>,
    event_tx: &'a broadcast::Sender<String>,
    last_check_at: &'a Arc<AtomicI64>,
    checker: Option<&'a dyn Checker>,
    health: &'a NetworkHealthHandle,
    notifier_ids: &'a [String],
    interval_secs: u64,
    /// Notification state: an `indeterminate` check never flips it (D7, Fase 3).
    was_up: bool,
    /// Last DB write timestamp, for the time-based sampling in `run_monitor_check`.
    last_write_at: i64,
    /// Last *observable* status, so a sustained `indeterminate` period is recorded
    /// once instead of on every check (M5, Fase 3).
    observed: ObservedStatus,
}

#[async_trait]
impl jitter::Cycle for MonitorCycle<'_> {
    async fn check(&mut self) {
        self.was_up = run_monitor_check(
            self.db,
            self.monitor,
            self.notifier_cache,
            self.event_tx,
            self.last_check_at,
            self.checker,
            self.health,
            self.was_up,
            self.notifier_ids,
            &mut self.last_write_at,
            &mut self.observed,
            self.interval_secs,
            self.enabled_monitors.load(Ordering::Relaxed) as usize,
            self.breaker,
        )
        .await;
    }
}

// ──── The actual check (moved from main.rs) ─────

/// Append `id` unless it is already collected, preserving the insertion order.
fn push_unique(ids: &mut Vec<String>, id: &str) {
    if !ids.iter().any(|known| known == id) {
        ids.push(id.to_string());
    }
}

/// Union of the notifiers linked to every monitor caught by the storm (MAYOR-2).
///
/// The aggregated alert replaces *all* the individual notifications of the
/// episode, so it must reach the notifiers of every affected monitor — not only
/// those of the monitor that happens to open the storm. The opener's cached list
/// is reused as-is (no extra query for it) and the rest is read from the DB.
async fn storm_notifier_ids(
    db: &Database,
    opener_ids: &[String],
    opener_fallback: Option<&str>,
    affected_ids: &[String],
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();

    for id in opener_ids {
        push_unique(&mut ids, id);
    }
    if let Some(id) = opener_fallback {
        push_unique(&mut ids, id);
    }

    for monitor_id in affected_ids {
        match db.get_monitor_notifier_ids(monitor_id).await {
            Ok(linked) => {
                for id in linked {
                    push_unique(&mut ids, &id);
                }
            }
            Err(e) => tracing::warn!(
                "Scheduler: cannot read the notifiers of monitor '{}': {}",
                monitor_id,
                e
            ),
        }
    }

    ids
}

/// Map a notification label to the breaker's signal kind (#72).
///
/// Delegates to `SignalKind::from_label` so the label ↔ kind mapping has a single
/// source of truth (m5); `down`/`up` are the storm episode, not signals.
fn signal_kind_of(kind: &str) -> Option<breaker::SignalKind> {
    breaker::SignalKind::from_label(kind)
}

/// Serialize the SSE `type:"check"` event for a monitor/check pair.
///
/// Pure and side-effect free so the wire shape is directly testable. The
/// `error_kind` key is ALWAYS present: it carries the serialized `FailureCause`
/// (`snake_case`, e.g. `connect_refused`) when the check has an attributable
/// cause, and `null` otherwise.
fn check_event_payload(monitor: &Monitor, check: &CheckResult) -> String {
    serde_json::json!({
        "type": "check",
        "monitor_id": monitor.id,
        "monitor_name": monitor.name,
        "status": check.status,
        "response_time_ms": check.response_time_ms,
        "error_message": check.error_message,
        "error_kind": check.error_kind,
        "checked_at": check.checked_at,
    })
    .to_string()
}

/// Run one check for a monitor and dispatch whatever notification it deserves.
///
/// `enabled_monitors` is the number of enabled monitors (heartbeats excluded) and
/// is the base of the storm breaker's percentage threshold. `breaker` is the
/// shared storm breaker (Fase 5): the decision for a `down`/`up` transition and,
/// since #72, for a `latency`/`expiry` signal is taken here, before dispatching,
/// and its lock is always released before any `await`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_monitor_check(
    db: &Database,
    monitor: &Monitor,
    notifier_cache: &Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: &broadcast::Sender<String>,
    last_check_at: &Arc<AtomicI64>,
    checker: Option<&dyn Checker>,
    health: &NetworkHealthHandle,
    was_up: bool,
    notifier_ids: &[String],
    last_write_at: &mut i64,
    observed: &mut ObservedStatus,
    interval_secs: u64,
    enabled_monitors: usize,
    breaker: &Arc<Mutex<StormBreaker>>,
) -> bool {
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    let checker = match checker {
        Some(c) => c,
        None => {
            tracing::warn!("Scheduler: no checker for type '{}'", monitor.monitor_type);
            return was_up;
        }
    };

    let outcome = checker.check(monitor).await;

    // ── Attribution (Fase 3, D6): combine the failure cause with the health of
    // our own egress. `indeterminate` means the failure is not the target's.
    let attributed = attribution::attribute(&outcome.status, outcome.cause, health.current());
    let is_indeterminate = attributed == attribution::INDETERMINATE;

    // Confirmation logic
    let mut effective_status = attributed.to_string();

    if !is_indeterminate {
        let is_up_raw = attributed == "up" || attributed == "warning";

        if !is_up_raw && monitor.confirmations_required > 0 {
            let new_failed = monitor.failed_checks + 1;
            let _ = db.set_failed_checks(&monitor.id, new_failed).await;

            if new_failed < monitor.confirmations_required {
                tracing::info!(
                    "Scheduler: {} failed check {}/{} (pending confirmation)",
                    monitor.name,
                    new_failed,
                    monitor.confirmations_required
                );
                effective_status = "error".into();
            } else {
                effective_status = "down".into();
            }
        } else if is_up_raw && monitor.failed_checks > 0 {
            let _ = db.reset_failed_checks(&monitor.id).await;
        }
    }

    let check = CheckResult {
        id: 0,
        monitor_id: monitor.id.clone(),
        status: effective_status,
        status_code: outcome.status_code,
        response_time_ms: outcome.response_time_ms as i64,
        error_message: outcome.error_message,
        checked_at: now_str,
        tls_cert_expires_at: outcome.tls.as_ref().and_then(|t| t.cert_expires_at.clone()),
        tls_cert_days_left: outcome.tls.as_ref().and_then(|t| t.cert_days_left),
        error_kind: outcome.cause.map(|cause| cause.as_str().to_string()),
    };

    // Write to DB when status changes OR at least once per interval_secs (time-based sampling)
    let is_up = check.status == "up" || check.status == "warning";
    let observed_now = ObservedStatus::from_status(&check.status, is_up);
    // M5: a sustained `indeterminate` period counts as a single change. The
    // transition is persisted and emitted once; the checks that follow fall back
    // to the sampling cadence. `Unknown` means "no check yet": the first check is
    // already covered by the first-check heuristic below.
    let status_changed = match *observed {
        ObservedStatus::Unknown => false,
        previous => previous != observed_now,
    };
    // Heuristic for the first check of this monitor after task startup:
    // monitor_task_inner initialises last_write_at to 0; the first check always
    // writes to DB (time_elapsed from epoch >= interval_secs) which then updates
    // last_write_at. So == 0 reliably means "first check of this run". We capture
    // it here, BEFORE the DB write below overwrites last_write_at.
    let is_first_check = *last_write_at == 0;
    let time_elapsed = now.timestamp() - *last_write_at;

    if status_changed || time_elapsed >= interval_secs as i64 {
        if let Err(e) = db.insert_check(&check).await {
            tracing::error!(
                "Scheduler: failed to save check for {}: {}",
                monitor.name,
                e
            );
            // N5: `observed` is deliberately left untouched, so the transition is
            // retried on the next check instead of being silently lost.
            return was_up;
        }
        *last_write_at = now.timestamp();
        // N5: the observed state advances only once the transition is durable.
        *observed = observed_now;
    }

    // Latency breach: emitted when a configured threshold is met (>=), independent
    // of status change. Only evaluated when the SSE block may run.
    let latency_breach = monitor
        .latency_threshold_ms
        .is_some_and(|t| check.response_time_ms >= t);

    // SSE event: only emit for RELEVANT events — the initial/first check of this
    // monitor, a status change, or a latency breach. A stable monitor (status
    // unchanged, latency within threshold) emits only the initial event, not one
    // per check. Receiver count is checked BEFORE serializing (zero-alloc when
    // nobody is listening). DB persistence (time-based sampling above) is untouched.
    if event_tx.receiver_count() > 0 && (is_first_check || status_changed || latency_breach) {
        let event = check_event_payload(monitor, &check);
        let _ = event_tx.send(event);
    }

    // Detect status changes and latency breaches
    let notification_type: Option<(&str, String)> = {
        if is_indeterminate {
            // D7: an unattributable failure is observability, not an incident.
            // It must not notify, not flip the monitor's notification state and
            // not count as a recovery.
            None
        } else if was_up && !is_up {
            let template = monitor
                .message_template_down
                .as_deref()
                .unwrap_or(template::defaults::DOWN);
            let ctx = TemplateContext::for_down(monitor, &check, "up");
            Some(("down", template::render_template(template, &ctx)))
        } else if !was_up && is_up {
            let template = monitor
                .message_template_up
                .as_deref()
                .unwrap_or(template::defaults::UP);
            let ctx = TemplateContext::for_up(monitor, &check, "down");
            Some(("up", template::render_template(template, &ctx)))
        } else if is_up {
            let expiry_notification = monitor
                .config_json
                .get("expiry_days")
                .and_then(|v| v.as_i64())
                .and_then(|expiry_days| {
                    outcome
                        .tls
                        .as_ref()
                        .and_then(|tls| tls.cert_days_left)
                        .and_then(|days_left| {
                            if days_left < expiry_days {
                                let template = monitor
                                    .message_template_expiry
                                    .as_deref()
                                    .unwrap_or(template::defaults::EXPIRY);
                                let ctx = TemplateContext::for_expiry(
                                    monitor,
                                    &check,
                                    days_left,
                                    expiry_days,
                                );
                                Some(("expiry", template::render_template(template, &ctx)))
                            } else {
                                None
                            }
                        })
                });

            if expiry_notification.is_some() {
                expiry_notification
            } else if let Some(threshold) = monitor.latency_threshold_ms {
                if check.response_time_ms > threshold {
                    let template = monitor
                        .message_template_latency
                        .as_deref()
                        .unwrap_or(template::defaults::LATENCY);
                    let ctx = TemplateContext::for_latency(monitor, &check, threshold);
                    Some(("latency", template::render_template(template, &ctx)))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    // ── Fase 5 (D7–D8): the storm breaker rules on this transition before anything
    // is dispatched. `down`/`up` take part in a storm and, since #72, so do the
    // `latency`/`expiry` signals (each with its own independent episode). The lock
    // is taken, the decision computed (synchronous work) and released — never held
    // across an `await`.
    let notif_kind = notification_type.as_ref().map(|(kind, _)| *kind);
    let signal = notif_kind.and_then(signal_kind_of);
    let (decision, threshold_percent) = {
        let mut guard = breaker.lock().await;
        // `decision_at`: do not shadow the outer `now` (the check timestamp).
        let decision_at = Instant::now();
        let decision = match notif_kind {
            Some("down") => {
                guard.on_down(decision_at, &monitor.id, &monitor.name, enabled_monitors)
            }
            Some("up") => guard.on_up(decision_at),
            _ => match signal {
                Some(kind) => guard.on_signal(
                    decision_at,
                    kind,
                    &monitor.id,
                    &monitor.name,
                    enabled_monitors,
                ),
                None => BreakerDecision::Pass,
            },
        };
        (decision, guard.config().threshold_percent)
    };

    let mut storm_ids: Option<Vec<String>> = None;
    let notification_type: Option<(&str, String)> = match decision {
        BreakerDecision::Pass => notification_type,
        // A storm is already being reported: this individual alert is dropped.
        BreakerDecision::Suppress => None,
        BreakerDecision::OpenAggregate(alert) => {
            // MAYOR-2: the aggregate replaces every individual notification of the
            // episode, so it goes to the *union* of the affected monitors'
            // notifiers. Using only the opener's would silence the whole storm when
            // the opener happens to have none.
            let ids = storm_notifier_ids(
                db,
                notifier_ids,
                monitor.notifier_id.as_deref(),
                &alert.monitor_ids,
            )
            .await;

            if ids.is_empty() {
                // A storm with no destination: a warning is better than N silent
                // individual alerts. The breaker deliberately stays `Open`, so the
                // episode is still deduped instead of re-alerting on every down.
                tracing::warn!(
                    "Scheduler: storm of {} monitors has no notifier to report to",
                    alert.count
                );
                None
            } else {
                // D7: render the aggregated template of the episode's kind. The
                // downs episode keeps `default_template_storm` (settings) and the
                // built-in fallback; the latency/expiry episodes (#72) use their
                // built-in `STORM_LATENCY`/`STORM_EXPIRY` (no new settings). Exactly
                // one aggregated message per storm. A failed read is logged (not
                // swallowed) and falls back too.
                let ctx = TemplateContext::for_storm(
                    alert.count,
                    &alert.monitors,
                    alert.window_seconds,
                    threshold_percent,
                );
                let (kind, message) = match signal {
                    Some(kind) => {
                        let template = match kind {
                            breaker::SignalKind::Latency => template::defaults::STORM_LATENCY,
                            breaker::SignalKind::Expiry => template::defaults::STORM_EXPIRY,
                        };
                        (kind.as_str(), template::render_template(template, &ctx))
                    }
                    None => {
                        let configured = match db.get_setting(breaker::SETTING_TEMPLATE_STORM).await
                        {
                            Ok(value) => value,
                            Err(e) => {
                                tracing::warn!(
                                    "Scheduler: cannot read '{}': {}",
                                    breaker::SETTING_TEMPLATE_STORM,
                                    e
                                );
                                None
                            }
                        };
                        let template = configured.as_deref().unwrap_or(template::defaults::STORM);
                        ("down", template::render_template(template, &ctx))
                    }
                };
                storm_ids = Some(ids);
                Some((kind, message))
            }
        }
    };

    // Send notification (using cached notifier_ids)
    if let Some((notif_type, message)) = notification_type {
        // The aggregated alert brings its own (union) target list (MAYOR-2); every
        // other notification goes to this monitor's cached notifiers.
        let ids = match storm_ids {
            Some(ids) => ids,
            None => {
                let mut ids = notifier_ids.to_vec();
                if ids.is_empty() {
                    if let Some(ref nid) = monitor.notifier_id {
                        ids.push(nid.clone());
                    }
                }
                ids
            }
        };

        let cache = notifier_cache.read().await;
        for nid in &ids {
            if let Some(notifier) = cache.get(nid) {
                if !notifier.enabled {
                    continue;
                }
                match notifier.notifier_type.as_str() {
                    "telegram" => {
                        let bot_token = notifier
                            .config_json
                            .get("bot_token")
                            .and_then(|v| v.as_str());
                        let chat_id = notifier.config_json.get("chat_id").and_then(|v| v.as_str());
                        if let (Some(token), Some(chat)) = (bot_token, chat_id) {
                            if let Err(e) = notifier::telegram::send_telegram_notification(
                                token, chat, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: telegram notification failed: {}", e);
                            }
                        }
                    }
                    "matrix" => {
                        let homeserver = notifier
                            .config_json
                            .get("homeserver_url")
                            .and_then(|v| v.as_str());
                        let access_token = notifier
                            .config_json
                            .get("access_token")
                            .and_then(|v| v.as_str());
                        let room_id = notifier.config_json.get("room_id").and_then(|v| v.as_str());
                        if let (Some(hs), Some(tok), Some(rid)) =
                            (homeserver, access_token, room_id)
                        {
                            if let Err(e) =
                                notifier::matrix::send_matrix_notification(hs, tok, rid, &message)
                                    .await
                            {
                                tracing::warn!("Scheduler: matrix notification failed: {}", e);
                            }
                        }
                    }
                    "ntfy" => {
                        let topic = notifier.config_json.get("topic").and_then(|v| v.as_str());
                        let server_url = notifier
                            .config_json
                            .get("server_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("https://ntfy.sh");
                        let token = notifier.config_json.get("token").and_then(|v| v.as_str());
                        if let Some(t) = topic {
                            if let Err(e) = notifier::ntfy::send_ntfy_notification(
                                t, server_url, token, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: ntfy notification failed: {}", e);
                            }
                        }
                    }
                    "webhook" => {
                        let url = notifier.config_json.get("url").and_then(|v| v.as_str());
                        let method = notifier
                            .config_json
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("POST");
                        let headers_json = notifier
                            .config_json
                            .get("headers")
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        if let Some(u) = url {
                            if let Err(e) = notifier::webhook::send_webhook_notification(
                                u,
                                method,
                                &headers_json,
                                &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: webhook notification failed: {}", e);
                            }
                        }
                    }
                    "slack" => {
                        let webhook_url = notifier
                            .config_json
                            .get("webhook_url")
                            .and_then(|v| v.as_str());
                        if let Some(u) = webhook_url {
                            if let Err(e) =
                                notifier::slack::send_slack_notification(u, &message).await
                            {
                                tracing::warn!("Scheduler: slack notification failed: {}", e);
                            }
                        }
                    }
                    "discord" => {
                        let webhook_url = notifier
                            .config_json
                            .get("webhook_url")
                            .and_then(|v| v.as_str());
                        if let Some(u) = webhook_url {
                            if let Err(e) =
                                notifier::discord::send_discord_notification(u, &message).await
                            {
                                tracing::warn!("Scheduler: discord notification failed: {}", e);
                            }
                        }
                    }
                    "email" => {
                        let smtp_host = notifier
                            .config_json
                            .get("smtp_host")
                            .and_then(|v| v.as_str());
                        let smtp_port = notifier
                            .config_json
                            .get("smtp_port")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(587) as u16;
                        let username = notifier
                            .config_json
                            .get("username")
                            .and_then(|v| v.as_str());
                        let password = notifier
                            .config_json
                            .get("password")
                            .and_then(|v| v.as_str());
                        let from = notifier.config_json.get("from").and_then(|v| v.as_str());
                        let to = notifier.config_json.get("to").and_then(|v| v.as_str());
                        if let (Some(host), Some(user), Some(pass), Some(f), Some(t)) =
                            (smtp_host, username, password, from, to)
                        {
                            if let Err(e) = notifier::email::send_email_notification(
                                host, smtp_port, user, pass, f, t, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: email notification failed: {}", e);
                            }
                        }
                    }
                    "gotify" => {
                        let server_url = notifier
                            .config_json
                            .get("server_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("http://localhost:8080");
                        let app_token = notifier
                            .config_json
                            .get("app_token")
                            .and_then(|v| v.as_str());
                        let priority = notifier
                            .config_json
                            .get("priority")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(5);
                        if let Some(t) = app_token {
                            if let Err(e) = notifier::gotify::send_gotify_notification(
                                server_url, t, priority, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: gotify notification failed: {}", e);
                            }
                        }
                    }
                    "valet" => {
                        let base_url = notifier
                            .config_json
                            .get("base_url")
                            .and_then(|v| v.as_str());
                        let ingest_token = notifier
                            .config_json
                            .get("ingest_token")
                            .and_then(|v| v.as_str());
                        let origin = notifier
                            .config_json
                            .get("origin")
                            .and_then(|v| v.as_str())
                            .unwrap_or("WatchBeat");
                        let author = &monitor.name;
                        let level = match notif_type {
                            "down" => "critical",
                            "latency" | "expiry" => "warning",
                            _ => "info",
                        };
                        if let (Some(i), Some(b)) = (ingest_token, base_url) {
                            if let Err(e) = notifier::valet::send_valet_notification(
                                b, i, origin, author, level, &message,
                            )
                            .await
                            {
                                tracing::warn!("Scheduler: valet notification failed: {}", e);
                            }
                        }
                    }
                    _ => tracing::warn!(
                        "Scheduler: unknown notifier type '{}'",
                        notifier.notifier_type
                    ),
                }
            }
        }
    }

    last_check_at.store(now.timestamp(), Ordering::Relaxed);
    if is_indeterminate {
        was_up
    } else {
        is_up
    }
}

// ──── Characterization tests (current behavior) ─────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::{CheckOutcome, FailureCause};
    use crate::jitter;
    use crate::netcheck::NetworkHealth;
    use async_trait::async_trait;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use tempfile::TempDir;

    struct AlwaysUpChecker;
    #[async_trait]
    impl Checker for AlwaysUpChecker {
        async fn check(&self, _monitor: &Monitor) -> CheckOutcome {
            CheckOutcome {
                status: "up".into(),
                status_code: Some(200),
                response_time_ms: 42,
                error_message: None,
                tls: None,
                cause: None,
            }
        }
    }

    struct AlwaysDownChecker;
    #[async_trait]
    impl Checker for AlwaysDownChecker {
        async fn check(&self, _monitor: &Monitor) -> CheckOutcome {
            CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: 0,
                error_message: Some("connection refused".into()),
                tls: None,
                cause: None,
            }
        }
    }

    fn sample_monitor(id: &str) -> Monitor {
        Monitor {
            id: id.into(),
            name: format!("Monitor {}", id),
            monitor_type: "http".into(),
            target: "https://example.com".into(),
            config_json: serde_json::json!({}),
            interval_seconds: 60,
            timeout_seconds: 30,
            enabled: true,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
            message_template_expiry: None,
            tags: vec![],
            token: None,
            grace_seconds: None,
            last_seen_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    async fn setup_db() -> (Database, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).await.unwrap();
        (db, dir)
    }

    /// A handle that never ran a cycle: attribution falls back to the base status.
    fn unknown_health() -> NetworkHealthHandle {
        NetworkHealthHandle::new()
    }

    /// No enabled monitors: the breaker's percentage threshold is unreachable, so
    /// the characterization tests keep observing the classic
    /// one-alert-por-monitor behaviour (Fase 5 keeps them untouched).
    const TEST_ENABLED_MONITORS: usize = 0;

    /// A breaker that can never declare a storm. See [`TEST_ENABLED_MONITORS`].
    fn test_breaker() -> Arc<Mutex<StormBreaker>> {
        Arc::new(Mutex::new(StormBreaker::new(BreakerConfig::default())))
    }

    /// Characterizes current sampling: with a stable status only the time-based
    /// write happens, i.e. at most one row per interval (N6).
    #[tokio::test]
    async fn test_time_based_sampling_writes_once_per_interval() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-sampling"))
            .await
            .unwrap();

        let checker = AlwaysUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;

        let mut was_up = true;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        // Run 15 checks with stable UP status
        for _ in 1..=15 {
            was_up = run_monitor_check(
                &db,
                &sample_monitor("m-sampling"),
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &unknown_health(),
                was_up,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                interval_secs,
                TEST_ENABLED_MONITORS,
                &test_breaker(),
            )
            .await;
        }

        // Time-based sampling: the whole loop runs well inside `interval_secs`, so
        // only the first check (which always writes) reaches the DB.
        let checks = db.get_checks("m-sampling", 100, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            1,
            "a stable monitor writes once per interval, not once per check"
        );
    }

    /// Characterizes that status changes bypass the sampling and write immediately.
    #[tokio::test]
    async fn test_status_change_writes_immediately_regardless_of_count() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-status"))
            .await
            .unwrap();

        let up_checker = AlwaysUpChecker;
        let down_checker = AlwaysDownChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;

        let mut was_up = true;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        // 3 checks UP (stable, none written because rapid succession)
        for _ in 1..=3 {
            was_up = run_monitor_check(
                &db,
                &sample_monitor("m-status"),
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&up_checker),
                &unknown_health(),
                was_up,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                interval_secs,
                TEST_ENABLED_MONITORS,
                &test_breaker(),
            )
            .await;
        }

        // Check #4: status changes UP→DOWN → should write immediately
        run_monitor_check(
            &db,
            &sample_monitor("m-status"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&down_checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let checks = db.get_checks("m-status", 100, 0).await.unwrap();
        // First call writes (time_elapsed from epoch >= interval_secs),
        // then status change writes immediately = 2 total
        assert_eq!(
            checks.len(),
            2,
            "First check writes by time, status change writes immediately"
        );
        assert_eq!(checks[0].status, "down");
        assert_eq!(checks[1].status, "up");
    }

    // ──── RED tests: new time-based sampling behavior ─────

    /// New behavior: writes when interval_secs have elapsed since last write.
    /// With old count-based code this FAILS because check #1 is not a multiple of 10.
    #[tokio::test]
    async fn test_time_based_sampling_writes_when_interval_elapsed() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-time-1"))
            .await
            .unwrap();

        let checker = AlwaysUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let mut last_write_at: i64 = 0; // epoch — time_elapsed will be >= interval_secs
        let mut observed = ObservedStatus::Unknown;
        let interval_secs: u64 = 60;

        let was_up = run_monitor_check(
            &db,
            &sample_monitor("m-time-1"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &unknown_health(),
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let checks = db.get_checks("m-time-1", 100, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            1,
            "Should write when interval_secs have elapsed since last write"
        );
        assert!(was_up);
    }

    /// New behavior: skips write when called rapidly within the same interval.
    #[tokio::test]
    async fn test_time_based_sampling_skips_within_interval() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-time-2"))
            .await
            .unwrap();

        let checker = AlwaysUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;

        // First call: last_write_at=0 → time_elapsed >= 60 → writes
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let was_up = run_monitor_check(
            &db,
            &sample_monitor("m-time-2"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &unknown_health(),
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        // Second call: last_write_at=now → time_elapsed=0 < 60 → no write
        run_monitor_check(
            &db,
            &sample_monitor("m-time-2"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let checks = db.get_checks("m-time-2", 100, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            1,
            "Only first check should write; second is within interval"
        );
    }

    /// New behavior: status change writes immediately regardless of elapsed time.
    #[tokio::test]
    async fn test_time_based_status_change_writes_immediately() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-time-3"))
            .await
            .unwrap();

        let up_checker = AlwaysUpChecker;
        let down_checker = AlwaysDownChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;

        // First call: writes (interval elapsed from epoch), status=UP
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let was_up = run_monitor_check(
            &db,
            &sample_monitor("m-time-3"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&up_checker),
            &unknown_health(),
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        // Second call: status changes UP→DOWN, writes immediately
        run_monitor_check(
            &db,
            &sample_monitor("m-time-3"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&down_checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let checks = db.get_checks("m-time-3", 100, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            2,
            "Both checks should write: first by time, second by status change"
        );
        assert_eq!(checks[0].status, "down");
        assert_eq!(checks[1].status, "up");
    }

    // ──── RED tests: SSE emission restricted to relevant events ─────

    /// Checker that always reports UP with a response time over any typical latency
    /// threshold (used to prove latency breaches emit SSE events).
    struct LatencyUpChecker;
    #[async_trait]
    impl Checker for LatencyUpChecker {
        async fn check(&self, _monitor: &Monitor) -> CheckOutcome {
            CheckOutcome {
                status: "up".into(),
                status_code: Some(200),
                response_time_ms: 200,
                error_message: None,
                tls: None,
                cause: None,
            }
        }
    }

    /// Drains all buffered SSE events from a broadcast receiver (non-blocking).
    async fn drain_events(rx: &mut broadcast::Receiver<String>) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
                Ok(Ok(s)) => out.push(s),
                // Preserve any lagged events; receiver is fast enough we shouldn't lag.
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                _ => break,
            }
        }
        out
    }

    /// A stable monitor (status unchanged, no latency breach) must emit ONLY the
    /// initial/first SSE event, not one per subsequent check.
    #[tokio::test]
    async fn test_stable_monitor_emits_only_initial_sse_event() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-sse-stable"))
            .await
            .unwrap();

        let checker = AlwaysUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, mut rx) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;
        let mut was_up = true;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        for _ in 1..=3 {
            was_up = run_monitor_check(
                &db,
                &sample_monitor("m-sse-stable"),
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &unknown_health(),
                was_up,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                interval_secs,
                TEST_ENABLED_MONITORS,
                &test_breaker(),
            )
            .await;
        }

        let events = drain_events(&mut rx).await;
        assert_eq!(
            events.len(),
            1,
            "Stable monitor should emit only the initial SSE event (got {:?})",
            events
        );
    }

    /// A status change (UP→DOWN) must emit an SSE event. Stable checks before it
    /// must NOT emit extra events: total = 1 initial + 1 status change = 2.
    #[tokio::test]
    async fn test_status_change_emits_sse_event() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-sse-status"))
            .await
            .unwrap();

        let up_checker = AlwaysUpChecker;
        let down_checker = AlwaysDownChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, mut rx) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;
        let mut was_up = true;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        // Check 1: UP (initial event)
        was_up = run_monitor_check(
            &db,
            &sample_monitor("m-sse-status"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&up_checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;
        // Check 2: UP again (stable — must NOT emit)
        was_up = run_monitor_check(
            &db,
            &sample_monitor("m-sse-status"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&up_checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;
        // Check 3: DOWN (status change → must emit)
        run_monitor_check(
            &db,
            &sample_monitor("m-sse-status"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&down_checker),
            &unknown_health(),
            was_up,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let events = drain_events(&mut rx).await;
        assert_eq!(
            events.len(),
            2,
            "Status change should emit initial + change = 2 events (got {:?})",
            events
        );
    }

    /// Feature `sse-error-kind`: the pure payload builder keeps the established
    /// fields and always emits the `error_kind` key.
    #[test]
    fn check_event_payload_shape_is_stable_and_always_carries_error_kind() {
        let monitor = sample_monitor("m-shape");
        let base = CheckResult {
            id: 0,
            monitor_id: monitor.id.clone(),
            status: "down".into(),
            status_code: None,
            response_time_ms: 12,
            error_message: Some("boom".into()),
            checked_at: "2026-01-01T00:00:00Z".into(),
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: Some(FailureCause::ConnectRefused.as_str().to_string()),
        };

        let with_cause: serde_json::Value =
            serde_json::from_str(&check_event_payload(&monitor, &base)).unwrap();
        assert_eq!(with_cause["type"], "check");
        assert_eq!(with_cause["monitor_id"], "m-shape");
        assert_eq!(with_cause["status"], "down");
        assert_eq!(with_cause["error_kind"], "connect_refused");

        let healthy = CheckResult {
            status: "up".into(),
            error_message: None,
            error_kind: None,
            ..base
        };
        let without_cause: serde_json::Value =
            serde_json::from_str(&check_event_payload(&monitor, &healthy)).unwrap();
        assert_eq!(
            without_cause.get("error_kind"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(
            without_cause.as_object().map(serde_json::Map::len),
            Some(8),
            "payload must keep exactly the established fields plus error_kind"
        );
    }

    /// Feature `sse-error-kind`: the `type:"check"` event must carry the
    /// serialized `FailureCause` as `error_kind` when the check has an
    /// attributable cause.
    #[tokio::test]
    async fn test_sse_check_event_carries_error_kind_on_failure() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-sse-kind-down"))
            .await
            .unwrap();

        let checker = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, mut rx) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        run_monitor_check(
            &db,
            &sample_monitor("m-sse-kind-down"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &unknown_health(),
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let events = drain_events(&mut rx).await;
        let payload: serde_json::Value =
            serde_json::from_str(events.first().expect("no SSE event emitted")).unwrap();
        assert_eq!(
            payload["error_kind"], "connect_refused",
            "error_kind must carry the serialized FailureCause (payload: {payload})"
        );
    }

    /// Feature `sse-error-kind`: a healthy check has no attributable cause, so
    /// `error_kind` must be present and `null` (the key is never omitted).
    #[tokio::test]
    async fn test_sse_check_event_carries_null_error_kind_when_healthy() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-sse-kind-up"))
            .await
            .unwrap();

        let checker = AlwaysUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, mut rx) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        run_monitor_check(
            &db,
            &sample_monitor("m-sse-kind-up"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &unknown_health(),
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        let events = drain_events(&mut rx).await;
        let payload: serde_json::Value =
            serde_json::from_str(events.first().expect("no SSE event emitted")).unwrap();
        assert_eq!(
            payload.get("error_kind"),
            Some(&serde_json::Value::Null),
            "error_kind key must be present and null for a healthy check (payload: {payload})"
        );
    }

    /// Every check that breaches the latency threshold must emit an SSE event.
    #[tokio::test]
    async fn test_latency_breach_emits_sse_event() {
        let (db, _dir) = setup_db().await;
        let mut mon = sample_monitor("m-sse-lat");
        mon.latency_threshold_ms = Some(100);
        db.create_monitor(&mon).await.unwrap();

        let checker = LatencyUpChecker;
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, mut rx) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec![];
        let interval_secs: u64 = 60;
        let mut was_up = true;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        for _ in 1..=3 {
            was_up = run_monitor_check(
                &db,
                &mon,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &unknown_health(),
                was_up,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                interval_secs,
                TEST_ENABLED_MONITORS,
                &test_breaker(),
            )
            .await;
        }

        let events = drain_events(&mut rx).await;
        assert_eq!(
            events.len(),
            3,
            "Every latency-breaching check should emit an SSE event (got {:?})",
            events
        );
    }

    // ──── Attribution (feature: network-attribution) ─────

    /// Checker that fails with a real cause, following the phase 2 contract
    /// (`status` is `status_for(cause)`).
    struct CauseFailureChecker {
        status: &'static str,
        cause: FailureCause,
    }

    #[async_trait]
    impl Checker for CauseFailureChecker {
        async fn check(&self, _monitor: &Monitor) -> CheckOutcome {
            CheckOutcome {
                status: self.status.into(),
                status_code: None,
                response_time_ms: 0,
                error_message: Some("boom".into()),
                tls: None,
                cause: Some(self.cause),
            }
        }
    }

    /// Loopback HTTP server that captures the first request it receives, plus a
    /// notifier cache wired to it. Lets a test observe whether a notification was
    /// actually dispatched.
    async fn webhook_notifier() -> (
        Arc<RwLock<HashMap<String, Notifier>>>,
        tokio::sync::oneshot::Receiver<String>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // reqwest may need an explicit rustls provider to build its client.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = vec![0u8; 4096];
                let mut total = 0usize;
                while let Ok(n) = stream.read(&mut buf[total..]).await {
                    if n == 0 {
                        break;
                    }
                    total += n;
                    if String::from_utf8_lossy(&buf[..total]).contains("POST /hook") {
                        break;
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&buf[..total]).to_string());
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                    .await;
            }
        });

        let notifier = Notifier {
            id: "n-hook".into(),
            name: "test webhook".into(),
            notifier_type: "webhook".into(),
            config_json: serde_json::json!({ "url": format!("http://{addr}/hook") }),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut cache = HashMap::new();
        cache.insert(notifier.id.clone(), notifier);
        (Arc::new(RwLock::new(cache)), rx)
    }

    /// Read one *full* HTTP request (headers plus `Content-Length` body) so the
    /// notification text itself can be asserted on.
    async fn read_full_request(stream: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;

        let mut buf = vec![0u8; 16384];
        let mut total = 0usize;
        while let Ok(n) = stream.read(&mut buf[total..]).await {
            if n == 0 {
                break;
            }
            total += n;
            if let Ok(s) = std::str::from_utf8(&buf[..total]) {
                if let Some(pos) = s.find("\r\n\r\n") {
                    let content_length = s[..pos]
                        .lines()
                        .find_map(|line| {
                            if line.to_lowercase().starts_with("content-length:") {
                                line.split(':').nth(1)?.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    if total >= pos + 4 + content_length {
                        break;
                    }
                }
            }
        }
        String::from_utf8_lossy(&buf[..total]).to_string()
    }

    /// A webhook listener serving `expected` requests (the single-shot helper
    /// above cannot serve a storm). Every body is reported through the channel.
    async fn webhook_notifier_multi(
        expected: usize,
    ) -> (
        Arc<RwLock<HashMap<String, Notifier>>>,
        tokio::sync::mpsc::Receiver<String>,
    ) {
        use tokio::io::AsyncWriteExt;

        let _ = rustls::crypto::ring::default_provider().install_default();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(expected.max(1));

        tokio::spawn(async move {
            for _ in 0..expected {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let request = read_full_request(&mut stream).await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                    .await;
                if tx.send(request).await.is_err() {
                    break;
                }
            }
        });

        let notifier = Notifier {
            id: "n-hook".into(),
            name: "test webhook".into(),
            notifier_type: "webhook".into(),
            config_json: serde_json::json!({ "url": format!("http://{addr}/hook") }),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut cache = HashMap::new();
        cache.insert(notifier.id.clone(), notifier);
        (Arc::new(RwLock::new(cache)), rx)
    }

    /// Fase 5 (D7–D8): when the storm opens, exactly one aggregated alert is
    /// dispatched to the opening monitor's notifiers, and the downs that follow
    /// while the breaker stays `Open` are suppressed.
    #[tokio::test]
    async fn a_storm_dispatches_one_aggregate_and_suppresses_what_follows() {
        let (db, _dir) = setup_db().await;
        // Five slots: the fifth is only ever used if the suppressed down *does*
        // notify, which is exactly what the final assertion rules out.
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(5).await;
        let checker = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let (event_tx, _) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let notifier_ids = vec!["n-hook".to_string()];

        // 4 enabled monitors and a 100 % threshold: all four must fall.
        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            threshold_percent: 100,
            ..BreakerConfig::default()
        })));
        let enabled_monitors = 4usize;
        let last_check_at = Arc::new(AtomicI64::new(0));

        // The three downs before the threshold pass individually.
        for i in 0..3 {
            let monitor = sample_monitor(&format!("m-storm-{i}"));
            db.create_monitor(&monitor).await.unwrap();
            let mut last_write_at: i64 = 0;
            let mut observed = ObservedStatus::Unknown;
            run_monitor_check(
                &db,
                &monitor,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &health,
                true,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                60,
                enabled_monitors,
                &breaker,
            )
            .await;

            let request = tokio::time::timeout(Duration::from_secs(5), request_rx.recv())
                .await
                .expect("individual down notification was not dispatched in time")
                .expect("notification channel closed");
            assert!(
                !request.contains("degradada"),
                "the pre-storm downs are notified individually: {request}"
            );
        }

        // The fourth down opens the storm: one single aggregated alert.
        let opener = sample_monitor("m-storm-3");
        db.create_monitor(&opener).await.unwrap();
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &opener,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            60,
            enabled_monitors,
            &breaker,
        )
        .await;

        let aggregate = tokio::time::timeout(Duration::from_secs(5), request_rx.recv())
            .await
            .expect("aggregated alert was not dispatched in time")
            .expect("notification channel closed");
        assert!(
            aggregate.contains("degradada"),
            "the storm opens with the aggregated template: {aggregate}"
        );
        assert!(
            aggregate.contains("4 monitores caídos"),
            "the aggregate carries the count: {aggregate}"
        );
        assert!(
            aggregate.contains("Monitor m-storm-0"),
            "the aggregate lists the affected monitors: {aggregate}"
        );

        // A fifth down while `Open` is suppressed: nothing else is sent.
        let suppressed = sample_monitor("m-storm-4");
        db.create_monitor(&suppressed).await.unwrap();
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &suppressed,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            60,
            enabled_monitors,
            &breaker,
        )
        .await;

        assert!(
            tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
                .await
                .is_err(),
            "a down inside the storm must not be notified (the listener is still \
             waiting for a further request)"
        );
    }

    /// Feature #72: las notificaciones de latencia pasan por el breaker — cada
    /// monitor brecha su umbral por debajo del agregado (Pass individual), el que
    /// cruza el umbral emite UNA única alerta agregada y las señales siguientes del
    /// mismo tipo se descartan (Suppress).
    #[tokio::test]
    async fn latency_notifications_are_grouped_by_the_breaker() {
        let (db, _dir) = setup_db().await;
        // Slots 1..=3: los avisos individuales pre-umbral. Slot 4: el agregado. El
        // quinto sólo se usaría si la señal suprimida notificase, que es lo que la
        // última aserción descarta.
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(5).await;
        let checker = LatencyUpChecker;
        let (event_tx, _) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let notifier_ids = vec!["n-hook".to_string()];

        // 4 monitores y umbral del 100 %: los cuatro deben brechar para agrupar.
        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            enabled: true,
            threshold_percent: 100,
            // Ventana amplia: el test no depende del reloj real (m1).
            window_seconds: 3600,
            min_monitors: 3,
        })));
        let enabled_monitors = 4usize;
        let last_check_at = Arc::new(AtomicI64::new(0));

        let check = |id: &str| {
            let mut monitor = sample_monitor(id);
            monitor.latency_threshold_ms = Some(100);
            monitor
        };

        // Los tres primeros pasan individualmente (por debajo del umbral).
        for i in 0..3 {
            let monitor = check(&format!("m-lat-{i}"));
            db.create_monitor(&monitor).await.unwrap();
            let mut last_write_at: i64 = 0;
            let mut observed = ObservedStatus::Unknown;
            run_monitor_check(
                &db,
                &monitor,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &health,
                true,
                &notifier_ids,
                &mut last_write_at,
                &mut observed,
                60,
                enabled_monitors,
                &breaker,
            )
            .await;

            let request = next_request(&mut request_rx, "aviso de latencia individual").await;
            assert!(
                request.contains("High latency"),
                "the pre-threshold signal is the individual latency notice: {request}"
            );
            assert!(
                !request.contains("Latencia degradada"),
                "pre-threshold latencies are notified individually: {request}"
            );
        }

        // El cuarto abre el episodio: una única alerta agregada de latencia.
        let opener = check("m-lat-3");
        db.create_monitor(&opener).await.unwrap();
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &opener,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            60,
            enabled_monitors,
            &breaker,
        )
        .await;

        let aggregate = next_request(&mut request_rx, "agregado de latencia").await;
        assert!(
            aggregate.contains("Latencia degradada"),
            "the latency episode opens with the aggregated template: {aggregate}"
        );
        assert!(
            aggregate.contains("4 monitores con latencia alta"),
            "the aggregate carries the count: {aggregate}"
        );
        assert!(
            aggregate.contains("Monitor m-lat-0"),
            "the aggregate lists the affected monitors: {aggregate}"
        );

        // Una quinta señal del mismo tipo dentro del episodio se suprime.
        let suppressed = check("m-lat-4");
        db.create_monitor(&suppressed).await.unwrap();
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &suppressed,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            60,
            enabled_monitors,
            &breaker,
        )
        .await;

        assert!(
            tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
                .await
                .is_err(),
            "a latency signal inside the episode must not be notified (the listener \
             is still waiting for a further request)"
        );
    }

    /// Checker whose TLS certificate is about to expire (`cert_days_left`).
    struct ExpiringCertChecker {
        days_left: i64,
    }

    #[async_trait]
    impl Checker for ExpiringCertChecker {
        async fn check(&self, _monitor: &Monitor) -> CheckOutcome {
            CheckOutcome {
                status: "up".into(),
                status_code: Some(200),
                response_time_ms: 10,
                error_message: None,
                tls: Some(crate::checker::tls::TlsInfo {
                    cert_expires_at: None,
                    cert_days_left: Some(self.days_left),
                }),
                cause: None,
            }
        }
    }

    /// One-shot check convenience for the routing tests: fresh bookkeeping, healthy
    /// network and the notification state before the check (`was_up = true`).
    #[allow(clippy::too_many_arguments)] // mirrors `run_monitor_check`
    async fn run_one_check(
        db: &Database,
        monitor: &Monitor,
        notifier_cache: &Arc<RwLock<HashMap<String, Notifier>>>,
        event_tx: &broadcast::Sender<String>,
        last_check_at: &Arc<AtomicI64>,
        checker: &dyn Checker,
        breaker: &Arc<Mutex<StormBreaker>>,
        enabled_monitors: usize,
        notifier_ids: &[String],
    ) {
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            db,
            monitor,
            notifier_cache,
            event_tx,
            last_check_at,
            Some(checker),
            &unknown_health(),
            true,
            notifier_ids,
            &mut last_write_at,
            &mut observed,
            60,
            enabled_monitors,
            breaker,
        )
        .await;
    }

    /// #72 / M1: las señales de **expiración** también pasan por el breaker: pasan
    /// individualmente por debajo del umbral, la que lo cruza emite UNA agregada de
    /// expiración y la siguiente se suprime — sin tocar el episodio de latencia.
    #[tokio::test]
    async fn expiry_notifications_are_grouped_by_the_breaker() {
        let (db, _dir) = setup_db().await;
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(5).await;
        let checker = ExpiringCertChecker { days_left: 5 };
        let (event_tx, _) = broadcast::channel(16);
        let notifier_ids = vec!["n-hook".to_string()];
        let last_check_at = Arc::new(AtomicI64::new(0));

        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            enabled: true,
            threshold_percent: 100,
            window_seconds: 3600,
            min_monitors: 3,
        })));
        let enabled_monitors = 4usize;

        let expiring = |id: &str| {
            let mut monitor = sample_monitor(id);
            // Certificado con 5 días para un umbral de 10 → señal de expiración.
            monitor.config_json = serde_json::json!({ "expiry_days": 10 });
            monitor
        };

        // Los tres primeros pasan individualmente (por debajo del umbral).
        for i in 0..3 {
            let monitor = expiring(&format!("m-exp-{i}"));
            db.create_monitor(&monitor).await.unwrap();
            run_one_check(
                &db,
                &monitor,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                &checker,
                &breaker,
                enabled_monitors,
                &notifier_ids,
            )
            .await;

            let request = next_request(&mut request_rx, "aviso de expiración individual").await;
            assert!(
                request.contains("Certificate expires in"),
                "the pre-threshold signal is the individual expiry notice: {request}"
            );
            assert!(
                !request.contains("Certificados próximos a expirar"),
                "pre-threshold expirations are notified individually: {request}"
            );
        }

        // El cuarto abre el episodio de expiración: una única agregada de su tipo.
        let opener = expiring("m-exp-3");
        db.create_monitor(&opener).await.unwrap();
        run_one_check(
            &db,
            &opener,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            &checker,
            &breaker,
            enabled_monitors,
            &notifier_ids,
        )
        .await;

        let aggregate = next_request(&mut request_rx, "agregado de expiración").await;
        assert!(
            aggregate.contains("Certificados próximos a expirar"),
            "the expiry episode opens with its own aggregated template: {aggregate}"
        );
        assert!(
            aggregate.contains("4 monitores con certificado"),
            "the aggregate carries the count: {aggregate}"
        );
        assert!(
            aggregate.contains("Monitor m-exp-0"),
            "the aggregate lists the affected monitors: {aggregate}"
        );

        // Y una quinta señal de expiración dentro del episodio se suprime.
        let suppressed = expiring("m-exp-4");
        db.create_monitor(&suppressed).await.unwrap();
        run_one_check(
            &db,
            &suppressed,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            &checker,
            &breaker,
            enabled_monitors,
            &notifier_ids,
        )
        .await;

        assert!(
            tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
                .await
                .is_err(),
            "an expiry signal inside the episode must not be notified (the listener \
             is still waiting for a further request)"
        );
    }

    /// Next notification body, or a panic if none arrives in time.
    async fn next_request(rx: &mut tokio::sync::mpsc::Receiver<String>, what: &str) -> String {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap_or_else(|_| panic!("{what}: no notification was dispatched in time"))
            .unwrap_or_else(|| panic!("{what}: the notification channel was closed"))
    }

    /// MENOR-4: an `up` during the storm is suppressed and notifies again once the
    /// breaker re-arms.
    ///
    /// The breaker's clock is injected by design (D1), so the re-arm is produced by
    /// feeding it an instant past the window — no real sleep in the test.
    #[tokio::test]
    async fn an_up_is_suppressed_during_the_storm_and_notifies_after_rearm() {
        let (db, _dir) = setup_db().await;
        // Slot 1: the aggregated alert. Slot 2: the recovery after the re-arm.
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(2).await;
        let down = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let up = AlwaysUpChecker;
        let (event_tx, _) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let linked = vec!["n-hook".to_string()];
        let unlinked: Vec<String> = vec![];

        // 3 monitors at 100 % → all three must fall. Only the opener carries a
        // notifier, so the storm produces exactly one request.
        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            threshold_percent: 100,
            ..BreakerConfig::default()
        })));
        let monitors: Vec<Monitor> = ["m-up-a", "m-up-b", "m-up-c"]
            .iter()
            .map(|id| sample_monitor(id))
            .collect();
        for monitor in &monitors {
            db.create_monitor(monitor).await.unwrap();
        }

        for (monitor, ids) in monitors
            .iter()
            .zip([&unlinked, &unlinked, &linked])
            .map(|(m, ids)| (m, ids.as_slice()))
        {
            let mut last_write_at: i64 = 0;
            let mut observed = ObservedStatus::Unknown;
            run_monitor_check(
                &db,
                monitor,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&down),
                &health,
                true,
                ids,
                &mut last_write_at,
                &mut observed,
                60,
                3,
                &breaker,
            )
            .await;
        }

        let aggregate = next_request(&mut request_rx, "aggregated alert").await;
        assert!(aggregate.contains("degradada"), "{aggregate}");

        // The opener recovers mid-storm: suppressed, nothing is sent.
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &monitors[2],
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&up),
            &health,
            false,
            &linked,
            &mut last_write_at,
            &mut observed,
            60,
            3,
            &breaker,
        )
        .await;

        assert!(
            tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
                .await
                .is_err(),
            "a recovery inside the storm must not be notified"
        );

        // The window elapses without new downs: the breaker re-arms and the next
        // recovery notifies again.
        breaker
            .lock()
            .await
            .on_up(Instant::now() + Duration::from_secs(3600));
        assert_eq!(
            breaker.lock().await.state(),
            crate::breaker::BreakerState::Closed
        );

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &monitors[2],
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&up),
            &health,
            false,
            &linked,
            &mut last_write_at,
            &mut observed,
            60,
            3,
            &breaker,
        )
        .await;

        let recovery = next_request(&mut request_rx, "recovery after the re-arm").await;
        assert!(
            recovery.contains("Recovered"),
            "the recovery notifies once the breaker re-armed: {recovery}"
        );
    }

    /// MAYOR-2 (guard): a storm whose monitors have no notifier at all sends
    /// nothing — a warning instead of N silent individual alerts — and the breaker
    /// stays `Open`, so the episode is still deduped instead of re-alerting.
    #[tokio::test]
    async fn a_storm_with_no_notifiers_anywhere_sends_nothing_and_stays_open() {
        let (db, _dir) = setup_db().await;
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(1).await;
        let checker = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let (event_tx, _) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let last_check_at = Arc::new(AtomicI64::new(0));

        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            threshold_percent: 100,
            ..BreakerConfig::default()
        })));
        let monitors: Vec<Monitor> = ["m-none-a", "m-none-b", "m-none-c"]
            .iter()
            .map(|id| sample_monitor(id))
            .collect();
        for monitor in &monitors {
            db.create_monitor(monitor).await.unwrap();
        }

        for monitor in &monitors {
            let mut last_write_at: i64 = 0;
            let mut observed = ObservedStatus::Unknown;
            run_monitor_check(
                &db,
                monitor,
                &notifier_cache,
                &event_tx,
                &last_check_at,
                Some(&checker),
                &health,
                true,
                &[],
                &mut last_write_at,
                &mut observed,
                60,
                3,
                &breaker,
            )
            .await;
        }

        assert_eq!(
            breaker.lock().await.state(),
            crate::breaker::BreakerState::Open,
            "the storm is declared even with nowhere to report it"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(300), request_rx.recv())
                .await
                .is_err(),
            "a storm with no destination must not notify (the listener is still waiting)"
        );
    }

    /// MAYOR-2: the aggregated alert must reach the notifiers of *every* affected
    /// monitor, not only those of the one that opened the storm. Here the opener
    /// has no notifiers at all and the fleet must not go silent.
    #[tokio::test]
    async fn the_aggregate_reaches_the_notifiers_of_every_affected_monitor() {
        let (db, _dir) = setup_db().await;
        // Two slots: m-b's individual down and, later, the aggregated alert.
        let (notifier_cache, mut request_rx) = webhook_notifier_multi(2).await;
        let checker = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let (event_tx, _) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let last_check_at = Arc::new(AtomicI64::new(0));

        // 3 monitors at 100 % → all three must fall. Only m-b is linked to a
        // notifier, and m-b is *not* the monitor opening the storm.
        let breaker = Arc::new(Mutex::new(StormBreaker::new(BreakerConfig {
            threshold_percent: 100,
            ..BreakerConfig::default()
        })));
        let monitors: Vec<Monitor> = ["m-a", "m-b", "m-c"]
            .iter()
            .map(|id| sample_monitor(id))
            .collect();
        for monitor in &monitors {
            db.create_monitor(monitor).await.unwrap();
        }
        // The link needs the notifier to exist in the DB (FK); the delivery itself
        // goes through the in-memory cache, which already holds it.
        db.create_notifier(
            "n-hook",
            "test webhook",
            "webhook",
            &serde_json::json!({}),
            true,
        )
        .await
        .unwrap();
        db.set_monitor_notifiers("m-b", &["n-hook".to_string()])
            .await
            .unwrap();

        // m-a: no notifiers at all → the first down passes and sends nothing.
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &monitors[0],
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &[],
            &mut last_write_at,
            &mut observed,
            60,
            3,
            &breaker,
        )
        .await;

        // m-b: its own individual down reaches its notifier.
        let linked = vec!["n-hook".to_string()];
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &monitors[1],
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &linked,
            &mut last_write_at,
            &mut observed,
            60,
            3,
            &breaker,
        )
        .await;
        let individual = next_request(&mut request_rx, "m-b individual down").await;
        assert!(!individual.contains("degradada"), "{individual}");

        // m-c opens the storm and has no notifiers of its own: the aggregate must
        // still be delivered through m-b's notifier, not dropped.
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        run_monitor_check(
            &db,
            &monitors[2],
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &[],
            &mut last_write_at,
            &mut observed,
            60,
            3,
            &breaker,
        )
        .await;

        let aggregate = next_request(&mut request_rx, "aggregated alert").await;
        assert!(
            aggregate.contains("degradada"),
            "the opener has no notifiers, the aggregate must use the union: {aggregate}"
        );
        assert!(
            aggregate.contains("Monitor m-b") && aggregate.contains("Monitor m-c"),
            "the aggregate lists every affected monitor: {aggregate}"
        );
    }

    /// Scenario: caída real con red sana se registra como down (y notifica).
    #[tokio::test]
    async fn a_real_failure_with_a_healthy_network_is_recorded_as_down_and_notifies() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-attr-target"))
            .await
            .unwrap();

        let (notifier_cache, request_rx) = webhook_notifier().await;
        let checker = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec!["n-hook".to_string()];
        let interval_secs: u64 = 60;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);

        let was_up = run_monitor_check(
            &db,
            &sample_monitor("m-attr-target"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        assert!(!was_up, "a real failure must leave the monitor not-up");
        let checks = db.get_checks("m-attr-target", 10, 0).await.unwrap();
        assert_eq!(checks[0].status, "down");
        assert_eq!(checks[0].error_kind.as_deref(), Some("connect_refused"));

        let request = tokio::time::timeout(Duration::from_secs(5), request_rx)
            .await
            .expect("down notification was not dispatched in time")
            .expect("notification channel closed");
        assert!(
            request.starts_with("POST /hook"),
            "unexpected notification request: {request}"
        );
    }

    /// Scenario: fallo por DNS degradado se registra como indeterminate, no
    /// notifica y no cambia el estado de notificación del monitor.
    #[tokio::test]
    async fn a_dns_failure_with_degraded_dns_is_recorded_as_indeterminate_without_notifying() {
        let (db, _dir) = setup_db().await;
        db.create_monitor(&sample_monitor("m-attr-indet"))
            .await
            .unwrap();

        let (notifier_cache, request_rx) = webhook_notifier().await;
        let checker = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };
        let (event_tx, _) = broadcast::channel(16);
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids = vec!["n-hook".to_string()];
        let interval_secs: u64 = 60;
        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::DnsDegraded);

        let was_up = run_monitor_check(
            &db,
            &sample_monitor("m-attr-indet"),
            &notifier_cache,
            &event_tx,
            &last_check_at,
            Some(&checker),
            &health,
            true,
            &notifier_ids,
            &mut last_write_at,
            &mut observed,
            interval_secs,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await;

        assert!(
            was_up,
            "indeterminate must not change the monitor's notification state"
        );
        let checks = db.get_checks("m-attr-indet", 10, 0).await.unwrap();
        assert_eq!(checks[0].status, "indeterminate");
        assert_eq!(
            checks[0].error_kind.as_deref(),
            Some("dns_timeout"),
            "the cause must be persisted even when the status is indeterminate"
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(200), request_rx)
                .await
                .is_err(),
            "indeterminate must not dispatch a down notification"
        );
    }

    // ──── M5/M7: secuencias de transición y no amplificación ─────

    /// Run one check with the given checker and health, threading the mutable
    /// per-monitor bookkeeping. No notifiers are wired: these tests observe
    /// persistence and SSE emission, not delivery.
    #[allow(clippy::too_many_arguments)]
    async fn run_check_with(
        db: &Database,
        monitor: &Monitor,
        event_tx: &broadcast::Sender<String>,
        checker: &dyn Checker,
        health: &NetworkHealthHandle,
        was_up: bool,
        last_write_at: &mut i64,
        observed: &mut ObservedStatus,
    ) -> bool {
        let notifier_cache = Arc::new(RwLock::new(HashMap::new()));
        let last_check_at = Arc::new(AtomicI64::new(0));
        let notifier_ids: Vec<String> = vec![];
        run_monitor_check(
            db,
            monitor,
            &notifier_cache,
            event_tx,
            &last_check_at,
            Some(checker),
            health,
            was_up,
            &notifier_ids,
            last_write_at,
            observed,
            60,
            TEST_ENABLED_MONITORS,
            &test_breaker(),
        )
        .await
    }

    /// Scenario M7: `indeterminate` como primer check. No altera el estado de
    /// notificación y queda registrado.
    #[tokio::test]
    async fn indeterminate_as_the_first_check_keeps_the_monitor_up_and_is_recorded() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-first-indet");
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, _rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::DnsDegraded);
        let checker = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &checker,
            &health,
            true,
            &mut last_write_at,
            &mut observed,
        )
        .await;

        assert!(was_up, "indeterminate must not flip the notification state");
        let checks = db.get_checks(&monitor.id, 10, 0).await.unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, "indeterminate");
    }

    /// Scenario M7: `up → indeterminate → up`. El fallo no atribuible no alerta
    /// ni cambia el estado de notificación, y cada transición queda registrada.
    #[tokio::test]
    async fn the_sequence_up_indeterminate_up_stays_up_and_records_every_transition() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-seq-up");
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, _rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let up = AlwaysUpChecker;
        let dns_failure = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let mut was_up = true;

        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &up,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        health.publish(NetworkHealth::DnsDegraded);
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &dns_failure,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        health.publish(NetworkHealth::Healthy);
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &up,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;

        assert!(
            was_up,
            "an unattributable blip must not flip the notification state"
        );
        let statuses: Vec<String> = db
            .get_checks(&monitor.id, 10, 0)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.status)
            .collect();
        assert_eq!(
            statuses.len(),
            3,
            "every transition must be persisted (got {statuses:?})"
        );
        assert!(
            statuses.iter().any(|s| s == "indeterminate"),
            "the unattributable check must be recorded (got {statuses:?})"
        );
    }

    /// Scenario M7: `down → indeterminate → down`. El estado de notificación
    /// permanece caído y cada transición queda registrada.
    #[tokio::test]
    async fn the_sequence_down_indeterminate_down_records_every_transition() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-seq-down");
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, _rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let down = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let dns_failure = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let mut was_up = true;

        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &down,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        health.publish(NetworkHealth::DnsDegraded);
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &dns_failure,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        health.publish(NetworkHealth::Healthy);
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &down,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;

        assert!(!was_up, "the monitor stays down throughout the sequence");
        let statuses: Vec<String> = db
            .get_checks(&monitor.id, 10, 0)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.status)
            .collect();
        assert_eq!(
            statuses,
            vec!["down", "indeterminate", "down"],
            "each transition must be persisted"
        );
    }

    /// Scenario M5: un corte sostenido de `indeterminate` no emite un evento SSE
    /// por check ni escribe una fila por check.
    #[tokio::test]
    async fn a_sustained_indeterminate_period_emits_a_single_event() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-sustained-indet");
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, mut rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let up = AlwaysUpChecker;
        let dns_failure = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let mut was_up = true;

        // Check 1: up (initial event, first write).
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &up,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        // Checks 2..=4: our egress stays down — one transition, then sampling.
        health.publish(NetworkHealth::DnsDegraded);
        for _ in 0..3 {
            was_up = run_check_with(
                &db,
                &monitor,
                &event_tx,
                &dns_failure,
                &health,
                was_up,
                &mut last_write_at,
                &mut observed,
            )
            .await;
        }

        assert!(was_up);
        let events = drain_events(&mut rx).await;
        assert_eq!(
            events.len(),
            2,
            "only the initial check and the transition emit an event (got {events:?})"
        );
        let checks = db.get_checks(&monitor.id, 10, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            2,
            "only the transition is written; the rest follow the sampling cadence"
        );
    }

    /// Scenario M7: `indeterminate` no incrementa ni resetea `failed_checks`
    /// aunque el monitor exija confirmaciones.
    #[tokio::test]
    async fn indeterminate_does_not_touch_the_confirmation_counter() {
        let (db, _dir) = setup_db().await;
        let mut monitor = sample_monitor("m-conf-indet");
        monitor.confirmations_required = 2;
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, _rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::DnsDegraded);
        let dns_failure = CauseFailureChecker {
            status: "error",
            cause: FailureCause::DnsTimeout,
        };

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;
        let was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &dns_failure,
            &health,
            true,
            &mut last_write_at,
            &mut observed,
        )
        .await;

        assert!(was_up);
        let stored = db.get_monitor(&monitor.id).await.unwrap().unwrap();
        assert_eq!(
            stored.failed_checks, 0,
            "indeterminate must not increment the confirmation counter"
        );

        // Contrast: a real failure does advance the counter (legacy behaviour).
        health.publish(NetworkHealth::Healthy);
        let real_failure = CauseFailureChecker {
            status: "down",
            cause: FailureCause::ConnectRefused,
        };
        let was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &real_failure,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;

        assert!(!was_up);
        let stored = db.get_monitor(&monitor.id).await.unwrap().unwrap();
        assert_eq!(
            stored.failed_checks, 1,
            "a real failure must still be counted"
        );
        let checks = db.get_checks(&monitor.id, 10, 0).await.unwrap();
        assert_eq!(
            checks[0].status, "error",
            "a pending confirmation is recorded as error"
        );
    }

    // ── N5: una escritura fallida no adelanta el estado observado ──

    /// Si el `insert_check` falla, la transición no puede darse por vista: el
    /// siguiente check debe reintentar la escritura en lugar de perderla.
    #[tokio::test]
    async fn a_failed_write_does_not_advance_the_observed_state() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-n5");
        db.create_monitor(&monitor).await.unwrap();

        let (event_tx, _rx) = broadcast::channel(16);
        let health = NetworkHealthHandle::new();
        health.publish(NetworkHealth::Healthy);
        let up = AlwaysUpChecker;
        let down = AlwaysDownChecker;

        let mut last_write_at: i64 = 0;
        let mut observed = ObservedStatus::Unknown;

        // Check 1: `up` — persisted, so `observed` advances and `last_write_at` is set.
        let mut was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &up,
            &health,
            true,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        assert!(was_up);

        // The monitor disappears: the next write violates the foreign key. The
        // failed write aborts the check, so the notification state is untouched.
        db.delete_monitor(&monitor.id).await.unwrap();
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &down,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        assert!(was_up, "a failed write aborts the check before notifying");

        // The monitor is back. The transition must still be pending, so this check
        // persists it. Had the failed write advanced `observed`, nothing would be
        // written: the sampling window has not elapsed.
        db.create_monitor(&monitor).await.unwrap();
        was_up = run_check_with(
            &db,
            &monitor,
            &event_tx,
            &down,
            &health,
            was_up,
            &mut last_write_at,
            &mut observed,
        )
        .await;
        assert!(!was_up);

        let checks = db.get_checks(&monitor.id, 10, 0).await.unwrap();
        assert_eq!(
            checks.len(),
            1,
            "the pending transition must be retried and persisted (got {checks:?})"
        );
        assert_eq!(checks[0].status, "down");
    }

    // ── Fase 4 · D5: el jitter no cambia el intervalo configurado ──

    /// Scenario: el `interval_seconds` configurado y persistido sigue igual.
    #[tokio::test]
    async fn the_jitter_should_not_change_the_configured_interval() {
        let (db, _dir) = setup_db().await;
        let monitor = sample_monitor("m-jitter-d5");
        db.create_monitor(&monitor).await.unwrap();

        // The same derivation the production cycle uses.
        let interval = Duration::from_secs(monitor.interval_seconds.max(1) as u64);
        let mut rng = StdRng::seed_from_u64(3);
        let offset = jitter::initial_offset(interval, &mut rng);
        let delay = jitter::next_delay(interval, &mut rng);

        assert!(offset < interval, "the offset is inside the interval");
        assert!(
            delay >= Duration::from_secs(54) && delay <= Duration::from_secs(66),
            "the wait is jittered, got {delay:?}"
        );
        assert_ne!(delay, interval, "the wait is not the plain interval");

        assert_eq!(
            monitor.interval_seconds, 60,
            "the in-memory value is untouched"
        );
        let stored = db.get_monitor("m-jitter-d5").await.unwrap().unwrap();
        assert_eq!(
            stored.interval_seconds, 60,
            "the persisted interval is untouched"
        );
    }
}
