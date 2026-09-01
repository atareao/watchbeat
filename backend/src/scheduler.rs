use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::future::FutureExt;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::checker;
use crate::checker::Checker;
use crate::db::Database;
use crate::models::{CheckResult, Monitor, Notifier};
use crate::notifier;
use crate::template::{self, TemplateContext};

// ───── Commands ─────

#[derive(Debug)]
pub enum SchedulerCommand {
    Spawn(Monitor),
    Update(Monitor),
    Remove(String),
    ReloadNotifiers,
    Shutdown,
}

// ───── Manager ─────

#[derive(Clone)]
pub struct SchedulerManager {
    pub active_tasks: Arc<AtomicU64>,
    pub last_check_at: Arc<AtomicI64>,
    tx: mpsc::Sender<SchedulerCommand>,
}

impl SchedulerManager {
    /// Start the scheduler. Loads existing monitors and begins monitoring.
    pub async fn spawn(db: Database, event_tx: broadcast::Sender<String>) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let active_tasks = Arc::new(AtomicU64::new(0));
        let last_check_at = Arc::new(AtomicI64::new(0));

        let mgr = Self {
            active_tasks: active_tasks.clone(),
            last_check_at: last_check_at.clone(),
            tx: tx.clone(),
        };

        tokio::spawn(async move {
            manager_loop(rx, db, event_tx, active_tasks, last_check_at).await;
        });

        mgr
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
}

// ──── Background manager loop ─────

async fn manager_loop(
    mut rx: mpsc::Receiver<SchedulerCommand>,
    db: Database,
    event_tx: broadcast::Sender<String>,
    active_tasks: Arc<AtomicU64>,
    last_check_at: Arc<AtomicI64>,
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
                    event_tx.clone(),
                    last_check_at.clone(),
                );
                task_map.insert(id, handle);
            }
        }
    }

    active_tasks.store(task_map.len() as u64, Ordering::Relaxed);

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
                        event_tx.clone(),
                        last_check_at.clone(),
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
            SchedulerCommand::Shutdown => {
                for (_, handle) in task_map.drain() {
                    handle.abort();
                }
                break;
            }
        }
        active_tasks.store(task_map.len() as u64, Ordering::Relaxed);
    }
}

// ──── Per-monitor task (with panic recovery) ─────

fn monitor_task(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<AtomicI64>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_secs = monitor.interval_seconds.max(1) as u64;
        loop {
            let result = AssertUnwindSafe(monitor_task_inner(
                db.clone(),
                monitor.clone(),
                notifier_cache.clone(),
                event_tx.clone(),
                last_check_at.clone(),
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

async fn monitor_task_inner(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<AtomicI64>,
    interval_secs: u64,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;

    // Cache checker once (reused across all checks for this monitor)
    let checker = checker::checker_for(&monitor);

    // Cache notifier IDs (avoids DB query on every status change)
    let notifier_ids = db
        .get_monitor_notifier_ids(&monitor.id)
        .await
        .unwrap_or_default();

    // Track previous status in memory (avoids get_latest_check query per check)
    let mut was_up = true;
    // Check counter: write every N checks to maintain latency data without writing every time
    let mut check_count: u64 = 0;

    loop {
        interval.tick().await;
        check_count = check_count.wrapping_add(1);
        was_up = run_monitor_check(
            &db,
            &monitor,
            &notifier_cache,
            &event_tx,
            &last_check_at,
            checker.as_deref(),
            was_up,
            &notifier_ids,
            check_count,
        )
        .await;
    }
}

// ──── The actual check (moved from main.rs) ─────

#[allow(clippy::too_many_arguments, clippy::manual_is_multiple_of)]
pub(crate) async fn run_monitor_check(
    db: &Database,
    monitor: &Monitor,
    notifier_cache: &Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: &broadcast::Sender<String>,
    last_check_at: &Arc<AtomicI64>,
    checker: Option<&dyn Checker>,
    was_up: bool,
    notifier_ids: &[String],
    check_count: u64,
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

    // Confirmation logic
    let is_up_raw = outcome.status == "up" || outcome.status == "warning";
    let mut effective_status = outcome.status.clone();

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
    };

    // Write to DB when status changes OR every 10th check (latency sampling)
    let is_up = check.status == "up" || check.status == "warning";
    let status_changed = was_up != is_up;

    if status_changed || (check_count % 10 == 0) {
        if let Err(e) = db.insert_check(&check).await {
            tracing::error!(
                "Scheduler: failed to save check for {}: {}",
                monitor.name,
                e
            );
            return was_up;
        }
    }

    // SSE event: only allocate JSON if someone is listening
    if event_tx.receiver_count() > 0 {
        let event = serde_json::json!({
            "type": "check",
            "monitor_id": monitor.id,
            "monitor_name": monitor.name,
            "status": check.status,
            "response_time_ms": check.response_time_ms,
            "error_message": check.error_message,
            "checked_at": check.checked_at,
        })
        .to_string();
        let _ = event_tx.send(event);
    }

    // Detect status changes and latency breaches
    let notification_type: Option<(&str, String)> = {
        if was_up && !is_up {
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

    // Send notification (using cached notifier_ids)
    if let Some((_notif_type, message)) = notification_type {
        let mut ids = notifier_ids.to_vec();
        if ids.is_empty() {
            if let Some(ref nid) = monitor.notifier_id {
                ids.push(nid.clone());
            }
        }

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
                    _ => tracing::warn!(
                        "Scheduler: unknown notifier type '{}'",
                        notifier.notifier_type
                    ),
                }
            }
        }
    }

    last_check_at.store(now.timestamp(), Ordering::Relaxed);
    is_up
}
