use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, RwLock};

use crate::checker;
use crate::db::Database;
use crate::models::{CheckResult, Monitor, Notifier};
use crate::notifier;
use crate::routes::metrics;
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
    pub last_check_at: Arc<RwLock<Option<String>>>,
    tx: mpsc::Sender<SchedulerCommand>,
}

impl SchedulerManager {
    /// Start the scheduler. Loads existing monitors and begins monitoring.
    pub async fn spawn(db: Database, event_tx: broadcast::Sender<String>) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let active_tasks = Arc::new(AtomicU64::new(0));
        let last_check_at = Arc::new(RwLock::new(None));

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
    last_check_at: Arc<RwLock<Option<String>>>,
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
                let handle = monitor_task_wrapper(
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
                    let handle = monitor_task_wrapper(
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

fn monitor_task_wrapper(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<RwLock<Option<String>>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let inner_handle = tokio::spawn(monitor_task_impl(
                db.clone(),
                monitor.clone(),
                notifier_cache.clone(),
                event_tx.clone(),
                last_check_at.clone(),
            ));

            match inner_handle.await {
                Ok(()) => {
                    // Normal exit — shouldn't happen with infinite loop,
                    // but if it does, exit the wrapper too.
                    break;
                }
                Err(e) if e.is_panic() => {
                    tracing::error!(
                        "Scheduler: panic in monitor task for '{}', restarting in 30s",
                        monitor.name
                    );
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    // Loop to restart
                }
                Err(_) => {
                    // Task was aborted via JoinHandle::abort() — exit gracefully
                    break;
                }
            }
        }
    })
}

async fn monitor_task_impl(
    db: Database,
    monitor: Monitor,
    notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: broadcast::Sender<String>,
    last_check_at: Arc<RwLock<Option<String>>>,
) {
    let mut interval =
        tokio::time::interval(Duration::from_secs(monitor.interval_seconds.max(1) as u64));
    // First tick is immediate — run check right away
    interval.tick().await;

    loop {
        interval.tick().await;
        run_monitor_check(&db, &monitor, &notifier_cache, &event_tx).await;

        // Update global last-check timestamp
        let now = chrono::Utc::now().to_rfc3339();
        *last_check_at.write().await = Some(now);
    }
}

// ──── The actual check (moved from main.rs) ─────

pub(crate) async fn run_monitor_check(
    db: &Database,
    monitor: &Monitor,
    notifier_cache: &Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: &broadcast::Sender<String>,
) {
    let was_up = match db.get_latest_check(&monitor.id).await {
        Ok(Some(c)) => c.status == "up",
        _ => true,
    };

    let checker = match checker::checker_for(monitor) {
        Some(c) => c,
        None => {
            tracing::warn!("Scheduler: no checker for type '{}'", monitor.monitor_type);
            return;
        }
    };

    let outcome = checker.check(monitor).await;

    let now_str = chrono::Utc::now().to_rfc3339();

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

    if let Err(e) = db.insert_check(&check).await {
        tracing::error!(
            "Scheduler: failed to save check for {}: {}",
            monitor.name,
            e
        );
        return;
    }

    metrics::inc_checks();

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

    // Detect status changes and latency breaches
    let is_up = check.status == "up" || check.status == "warning";

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

    // Send notification
    if let Some((_notif_type, message)) = notification_type {
        let mut notifier_ids = db
            .get_monitor_notifier_ids(&monitor.id)
            .await
            .unwrap_or_default();

        if notifier_ids.is_empty() {
            if let Some(ref nid) = monitor.notifier_id {
                notifier_ids.push(nid.clone());
            }
        }

        let cache = notifier_cache.read().await;
        for nid in &notifier_ids {
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
}
