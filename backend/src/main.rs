use std::sync::Arc;

use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use watchbeat::auth::{self, AppState, JwtValidator, SchedulerStatus};
use watchbeat::checker;
use watchbeat::config::Config;
use watchbeat::db::Database;
use watchbeat::embed::serve_embedded;
use watchbeat::models::CheckResult;
use watchbeat::notifier;
use watchbeat::routes;
use watchbeat::routes::metrics;
use watchbeat::template::{self, TemplateContext};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = Config::load();

    // ───── Tracing ─────
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_format == "json" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true),
            )
            .init();
    }

    tracing::info!("🚀 WatchBeat starting...");

    // ───── Connectivity verification ─────
    tracing::info!("🔌 Checking connectivity...");
    let check_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    match check_client.get("https://1.1.1.1").send().await {
        Ok(_) => tracing::info!("✅ Internet connectivity OK"),
        Err(e) => tracing::warn!("⚠️  Internet connectivity check failed: {} (non-fatal)", e),
    }
    match check_client
        .get(format!(
            "{}/.well-known/openid-configuration",
            config.oidc_issuer_url.trim_end_matches('/')
        ))
        .send()
        .await
    {
        Ok(_) => tracing::info!("✅ OIDC provider reachable"),
        Err(e) => tracing::warn!("⚠️  OIDC provider not reachable: {} (will retry)", e),
    }

    // ───── Data directory ─────
    if let Err(e) = tokio::fs::create_dir_all(&config.data_dir).await {
        tracing::warn!("Could not create data dir: {}", e);
    }

    // ───── Database ─────
    let db = match Database::open(&config.database_url).await {
        Ok(db) => {
            tracing::info!("📦 Database opened: {}", config.database_url.display());
            db
        }
        Err(e) => {
            tracing::error!("❌ Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    // ───── OIDC (mandatory) ─────
    let oidc_metadata = match auth::discover_oidc(&config).await {
        Ok(m) => {
            tracing::info!("✅ OIDC discovery: {}", m.issuer);
            m
        }
        Err(e) => {
            tracing::error!("❌ OIDC discovery failed: {}", e);
            std::process::exit(1);
        }
    };

    // ───── JWKS (como populates) ─────
    let jwt_validator = JwtValidator::new(&config.oidc_issuer_url, &config.oidc_client_id);
    if let Err(e) = jwt_validator.fetch_jwks(&config.oidc_issuer_url).await {
        tracing::error!("❌ JWKS fetch failed: {}. OIDC will not work.", e);
        std::process::exit(1);
    }
    let jwt_validator = Arc::new(jwt_validator);

    // ───── App State ─────
    let scheduler_status: Arc<Mutex<SchedulerStatus>> =
        Arc::new(Mutex::new(SchedulerStatus::default()));
    let (event_tx, _) = tokio::sync::broadcast::channel(auth::SSE_CHANNEL_CAPACITY);

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db: db.clone(),
        oidc_metadata: Some(oidc_metadata),
        jwt_validator: jwt_validator.clone(),
        oidc_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        scheduler_status: scheduler_status.clone(),
        event_tx: event_tx.clone(),
    });

    // ───── Scheduler ─────
    let db_for_scheduler = db.clone();
    let sched_status = scheduler_status.clone();
    let event_tx_for_sched = event_tx.clone();
    let retention_days = config.retention_days;
    tokio::spawn(async move {
        scheduler_loop(db_for_scheduler, sched_status, event_tx_for_sched, retention_days).await;
    });

    // ───── Consolidation Loop ─────
    let db_for_consolidation = db.clone();
    tokio::spawn(async move {
        consolidation_loop(db_for_consolidation).await;
    });

    // ───── Router (como populates) ─────
    let state_for_middleware = app_state.clone();
    let app = routes::api_routes()
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let state = state_for_middleware.clone();
                async move {
                    req.extensions_mut().insert(state);
                    watchbeat::auth::require_auth(req, next).await
                }
            },
        ))
        .fallback(|req: axum::extract::Request| async move {
            let path = req.uri().path().to_string();
            serve_embedded(&path).await
        })
        .with_state(app_state);

    let addr = if config.host == "0.0.0.0" {
        format!("[::]:{}", config.port)
    } else {
        format!("{}:{}", config.host, config.port)
    };

    tracing::info!("🌐 WatchBeat en http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

// ───── Scheduler Loop ─────

async fn scheduler_loop(
    db: Database,
    sched_status: Arc<Mutex<SchedulerStatus>>,
    event_tx: tokio::sync::broadcast::Sender<String>,
    retention_days: i64,
) {
    // Small delay to let the server start
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Track last check time per monitor to detect scheduler stalls
    let mut last_global_run = chrono::Utc::now();

    loop {
        // Spawn each iteration in its own task so a panic doesn't kill the scheduler
        let db = db.clone();
        let sched_status = sched_status.clone();
        let event_tx = event_tx.clone();
        let handle = tokio::spawn(async move {
            scheduler_iteration(&db, &sched_status, &event_tx, retention_days).await;
        });

        match handle.await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("🔥 Scheduler iteration panicked: {}. Restarting...", e);
            }
        }

        // Detect if scheduler has been stalled (no iterations for >5min)
        let now = chrono::Utc::now();
        let since_last = (now - last_global_run).num_seconds();
        if since_last > 300 {
            tracing::warn!(
                "⚠️ Scheduler was stalled for {}s — possible previous panic",
                since_last
            );
        }
        last_global_run = now;

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}

async fn scheduler_iteration(
    db: &Database,
    sched_status: &Arc<Mutex<SchedulerStatus>>,
    event_tx: &tokio::sync::broadcast::Sender<String>,
    retention_days: i64,
) {
    let monitors = match db.list_monitors().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Scheduler: failed to load monitors: {}", e);
            return;
        }
    };

    let notifiers = match db.list_notifiers().await {
        Ok(n) => n
            .into_iter()
            .map(|n| (n.id.clone(), n))
            .collect::<std::collections::HashMap<_, _>>(),
        Err(e) => {
            tracing::warn!("Scheduler: failed to load notifiers: {}", e);
            std::collections::HashMap::new()
        }
    };

    let mut checks_done = 0u64;

    // Run each monitor check in its own task so one slow monitor doesn't block others
    let check_handles: Vec<_> = monitors
        .iter()
        .filter(|m| m.enabled && m.monitor_type != "heartbeat")
        .map(|monitor| {
            let db = db.clone();
            let notifiers = notifiers.clone();
            let event_tx = event_tx.clone();
            let monitor = monitor.clone();
            tokio::spawn(async move {
                check_if_due(&db, &monitor, &notifiers, &event_tx).await;
            })
        })
        .collect();

    // Wait for all checks to complete (with individual timeouts handled by checker)
    for handle in check_handles {
        match handle.await {
            Ok(()) => checks_done += 1,
            Err(e) => tracing::warn!("Scheduler: monitor check task failed: {}", e),
        }
    }

    // Update scheduler status
    {
        let mut status = sched_status.lock().await;
        status.last_run_at = Some(chrono::Utc::now().to_rfc3339());
        status.last_monitors_checked = checks_done;
    }

    // Update prometheus monitor counts
    if let Ok(summaries) = db.get_monitor_summaries().await {
        let up = summaries
            .iter()
            .filter(|s| s.last_status.as_deref() == Some("up"))
            .count() as u64;
        let down = summaries
            .iter()
            .filter(|s| s.last_status.as_deref() == Some("down"))
            .count() as u64;
        metrics::set_monitor_counts(up, down);
    }

    // Cleanup old checks (configurable retention, default 30 days)
    if let Err(e) = db.cleanup_old_checks(retention_days).await {
        tracing::warn!("Scheduler: cleanup failed: {}", e);
    }
}

async fn check_if_due(
    db: &Database,
    monitor: &watchbeat::models::Monitor,
    notifiers: &std::collections::HashMap<String, watchbeat::models::Notifier>,
    event_tx: &tokio::sync::broadcast::Sender<String>,
) {
    let last_check = match db.get_latest_check(&monitor.id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            // Never checked — run now
            run_monitor_check(db, monitor, notifiers, event_tx).await;
            return;
        }
        Err(e) => {
            tracing::warn!(
                "Scheduler: failed to get last check for {}: {}",
                monitor.name,
                e
            );
            return;
        }
    };

    let last_time = match chrono::DateTime::parse_from_rfc3339(&last_check.checked_at) {
        Ok(t) => t.with_timezone(&chrono::Utc),
        Err(_) => {
            // Invalid timestamp — run now
            run_monitor_check(db, monitor, notifiers, event_tx).await;
            return;
        }
    };

    let elapsed = chrono::Utc::now() - last_time;
    let interval = chrono::Duration::seconds(monitor.interval_seconds);

    if elapsed >= interval {
        run_monitor_check(db, monitor, notifiers, event_tx).await;
    }
}

async fn run_monitor_check(
    db: &Database,
    monitor: &watchbeat::models::Monitor,
    notifiers: &std::collections::HashMap<String, watchbeat::models::Notifier>,
    event_tx: &tokio::sync::broadcast::Sender<String>,
) {
    let was_up = match db.get_latest_check(&monitor.id).await {
        Ok(Some(c)) => c.status == "up",
        _ => true, // Assume up if no history
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

    // ── Confirmation logic: if check failed but confirmations_required > 0,
    //    increment failed_checks and only mark DOWN once threshold reached.
    let is_up_raw = outcome.status == "up" || outcome.status == "warning";
    let mut effective_status = outcome.status.clone();

    if !is_up_raw && monitor.confirmations_required > 0 {
        let new_failed = monitor.failed_checks + 1;
        let _ = db.set_failed_checks(&monitor.id, new_failed).await;

        if new_failed < monitor.confirmations_required {
            // Not yet confirmed — record as 'error' (intermediate), don't alert DOWN
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
    } else if is_up_raw {
        // Reset counter on success
        if monitor.failed_checks > 0 {
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
    };

    if let Err(e) = db.insert_check(&check).await {
        tracing::error!(
            "Scheduler: failed to save check for {}: {}",
            monitor.name,
            e
        );
        return;
    }

    // Update prometheus metrics
    metrics::inc_checks();

    // Broadcast SSE event
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

    // ── Detect status changes and latency breaches, render templates, notify ──
    let is_up = check.status == "up" || check.status == "warning";

    // Determine notification type and render template
    let notification_type: Option<(&str, String)> = {
        // DOWN transition
        if was_up && !is_up {
            let template = monitor
                .message_template_down
                .as_deref()
                .unwrap_or(template::defaults::DOWN);
            let ctx = TemplateContext::for_down(monitor, &check, "up");
            Some(("down", template::render_template(template, &ctx)))
        }
        // UP transition (recovery from DOWN or LATENCY)
        else if !was_up && is_up {
            let template = monitor
                .message_template_up
                .as_deref()
                .unwrap_or(template::defaults::UP);
            let ctx = TemplateContext::for_up(monitor, &check, "down");
            Some(("up", template::render_template(template, &ctx)))
        }
        // Monitor is up — check for TLS expiry or latency breach
        else if is_up {
            // TLS certificate expiry takes priority over latency
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
            }
            // Latency threshold breach
            else if let Some(threshold) = monitor.latency_threshold_ms {
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

    // Send notification if we have a rendered message
    if let Some((_notif_type, message)) = notification_type {
        let mut notifier_ids = db
            .get_monitor_notifier_ids(&monitor.id)
            .await
            .unwrap_or_default();

        // Fallback to the legacy single-notifier field if the many-to-many
        // monitor_notifiers table is empty (e.g. monitors created before the
        // table existed, or migration gaps)
        if notifier_ids.is_empty() {
            if let Some(ref nid) = monitor.notifier_id {
                notifier_ids.push(nid.clone());
            }
        }

        for nid in &notifier_ids {
            if let Some(notifier) = notifiers.get(nid) {
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

// ───── Consolidation Loop ─────
// Runs every hour, grouping checks into 80 buckets per period (6h, 12h, 24h, 7d, 15d, 30d, 3m, 6m, 1a)
// and upserts them into consolidated_metrics for fast timeline queries.

const PERIODS: &[(&str, i64)] = &[
    ("6h", 6 * 3600),
    ("12h", 12 * 3600),
    ("24h", 24 * 3600),
    ("7d", 7 * 86400),
    ("15d", 15 * 86400),
    ("30d", 30 * 86400),
    ("3m", 90 * 86400),
    ("6m", 180 * 86400),
    ("1a", 365 * 86400),
];

const TARGET_BLOCKS: usize = 80;

async fn consolidation_loop(db: Database) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
    // Skip the immediate first tick — no data yet on startup
    ticker.tick().await;
    loop {
        ticker.tick().await;
        tracing::info!("Starting metric consolidation cycle");

        let monitors = match db.list_monitors().await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Failed to list monitors for consolidation: {e}");
                continue;
            }
        };

        let now = chrono::Utc::now();
        let one_hour_ago = (now - chrono::Duration::hours(1)).to_rfc3339();
        let mut total_buckets = 0usize;

        for monitor in &monitors {
            // Get checks from the last hour
            let checks = match db.get_timeline(&monitor.id, &one_hour_ago).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Consolidation: failed to get timeline for monitor {}: {e}",
                        monitor.id
                    );
                    continue;
                }
            };

            if checks.is_empty() {
                continue;
            }

            for &(period_label, period_secs) in PERIODS {
                let period_end = now;
                let period_start = now - chrono::Duration::seconds(period_secs);
                let total_span_secs = (period_end - period_start).num_seconds().max(1);
                let bucket_size_secs =
                    (total_span_secs as f64 / TARGET_BLOCKS as f64).ceil().max(1.0) as i64;

                let period_start_ts = period_start.timestamp();

                for i in 0..TARGET_BLOCKS {
                    let bucket_start_ts = period_start_ts + (i as i64 * bucket_size_secs);
                    let bucket_end_ts = bucket_start_ts + bucket_size_secs;

                    let bucket_start_str = chrono::DateTime::from_timestamp(bucket_start_ts, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default();
                    let bucket_end_str = chrono::DateTime::from_timestamp(bucket_end_ts, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default();

                    let bucket_points: Vec<_> = checks
                        .iter()
                        .filter(|p| p.checked_at >= bucket_start_str && p.checked_at < bucket_end_str)
                        .collect();

                    let count = bucket_points.len() as i64;

                    let (up_pct, avg_rt) = if count > 0 {
                        let up_count =
                            bucket_points.iter().filter(|p| p.status == "up").count() as i64;
                        let up = (up_count as f64 / count as f64) * 100.0;
                        let avg = bucket_points
                            .iter()
                            .filter_map(|p| p.response_time_ms)
                            .map(|v| v as f64)
                            .sum::<f64>()
                            / count as f64;
                        (up, avg)
                    } else {
                        (0.0, 0.0)
                    };

                    let bucket = watchbeat::models::ConsolidatedBucket {
                        monitor_id: monitor.id.clone(),
                        period: period_label.to_string(),
                        bucket_start: bucket_start_str,
                        up_pct,
                        avg_response_time_ms: avg_rt,
                        count,
                    };

                    if let Err(e) = db.insert_consolidated_bucket(&bucket).await {
                        tracing::warn!(
                            "Consolidation: failed to insert bucket for monitor {}: {e}",
                            monitor.id
                        );
                    } else {
                        total_buckets += 1;
                    }
                }
            }
        }

        tracing::info!(
            "Consolidation complete: {} monitors processed, {} buckets written",
            monitors.len(),
            total_buckets
        );
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => { tracing::info!("🛑 SIGINT received, shutting down..."); }
        _ = terminate => { tracing::info!("🛑 SIGTERM received, shutting down..."); }
    }
}
