use std::sync::Arc;

use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use vigilatrs::auth::{self, AppState, JwtValidator, SchedulerStatus};
use vigilatrs::checker;
use vigilatrs::config::Config;
use vigilatrs::db::Database;
use vigilatrs::embed::serve_embedded;
use vigilatrs::models::CheckResult;
use vigilatrs::notifier;
use vigilatrs::routes;

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

    tracing::info!("🚀 Vigilatrs starting...");

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

    // ───── JWKS ─────
    let jwt_validator = JwtValidator::new(&config.oidc_issuer_url, &config.oidc_client_id);
    if let Err(e) = jwt_validator.fetch_jwks(&oidc_metadata.jwks_uri).await {
        tracing::error!("❌ JWKS fetch failed: {}", e);
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
        oidc_states: Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        scheduler_status: scheduler_status.clone(),
        event_tx: event_tx.clone(),
    });

    // ───── Scheduler ─────
    let db_for_scheduler = db.clone();
    let sched_status = scheduler_status.clone();
    let event_tx_for_sched = event_tx.clone();
    tokio::spawn(async move {
        scheduler_loop(db_for_scheduler, sched_status, event_tx_for_sched).await;
    });

    // ───── Router ─────
    let state_for_middleware = app_state.clone();
    let app = routes::api_routes()
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let state = state_for_middleware.clone();
                async move {
                    let path = req.uri().path().to_string();
                    req.extensions_mut().insert(state);
                    if is_public_path(&path) {
                        return Ok(next.run(req).await);
                    }
                    vigilatrs::auth::require_auth(req, next).await
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

    tracing::info!("🌐 Vigilatrs en http://{}", addr);

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
) {
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    loop {
        let monitors = match db.list_monitors().await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Scheduler: failed to load monitors: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };

        let notifiers = match db.list_notifiers().await {
            Ok(n) => n.into_iter().map(|n| (n.id.clone(), n)).collect::<std::collections::HashMap<_, _>>(),
            Err(e) => {
                tracing::warn!("Scheduler: failed to load notifiers: {}", e);
                std::collections::HashMap::new()
            }
        };

        let mut checks_done = 0u64;

        for monitor in &monitors {
            if !monitor.enabled {
                continue;
            }

            // Check if it's time to run
            let last_check = match db.get_latest_check(&monitor.id).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    // Never checked — run now
                    run_monitor_check(&db, monitor, &notifiers, &event_tx).await;
                    checks_done += 1;
                    continue;
                }
                Err(e) => {
                    tracing::warn!("Scheduler: failed to get last check for {}: {}", monitor.name, e);
                    continue;
                }
            };

            // Parse last check time and compare with interval
            let last_time = match chrono::DateTime::parse_from_rfc3339(&last_check.checked_at) {
                Ok(t) => t.with_timezone(&chrono::Utc),
                Err(_) => {
                    // Invalid timestamp — run now
                    run_monitor_check(&db, monitor, &notifiers, &event_tx).await;
                    checks_done += 1;
                    continue;
                }
            };

            let elapsed = chrono::Utc::now() - last_time;
            let interval = chrono::Duration::seconds(monitor.interval_seconds);

            if elapsed >= interval {
                run_monitor_check(&db, monitor, &notifiers, &event_tx).await;
                checks_done += 1;
            }
        }

        // ── Heartbeat monitoring ──
        if let Ok(hbs) = db.list_heartbeats().await {
            let now = chrono::Utc::now();
            for hb in &hbs {
                // Skip if no notifier configured (nothing to alert)
                let hb_status_ok = hb.status == "ok" || hb.status == "pending";
                let grace_expired = match &hb.last_seen_at {
                    Some(ts) => {
                        chrono::DateTime::parse_from_rfc3339(ts)
                            .map(|t| now - t.with_timezone(&chrono::Utc) > chrono::Duration::seconds(hb.grace_seconds))
                            .unwrap_or(false)
                    }
                    None => true, // Never seen — consider missing if pending too long
                };

                if hb_status_ok && grace_expired {
                    tracing::info!("Heartbeat '{}' missed — grace period expired", hb.name);
                    if let Some(nid) = &hb.notifier_id {
                        if let Some(notifier) = notifiers.get(nid) {
                            if notifier.enabled && notifier.notifier_type == "telegram" {
                                let bot_token = notifier.config_json.get("bot_token").and_then(|v| v.as_str());
                                let chat_id = notifier.config_json.get("chat_id").and_then(|v| v.as_str());
                                if let (Some(token), Some(chat)) = (bot_token, chat_id) {
                                    let msg = format!(
                                        "🔴 Heartbeat '{}' no ha latido en {}s — posible fallo de cron/backup",
                                        hb.name, hb.grace_seconds
                                    );
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                    let _ = reqwest::Client::new().post(&url)
                                        .json(&serde_json::json!({"chat_id": chat, "text": msg}))
                                        .send().await;
                                }
                            }
                        }
                    }
                    // Mark as missing to avoid repeated alerts
                    let _ = db.upsert_heartbeat(&hb.id, &vigilatrs::models::Heartbeat {
                        status: "missing".into(),
                        ..hb.clone()
                    }).await;
                }
            }
        }

        // Update scheduler status
        {
            let mut status = sched_status.lock().await;
            status.last_run_at = Some(chrono::Utc::now().to_rfc3339());
            status.last_monitors_checked = checks_done;
        }

        // Cleanup old checks (configurable retention, default 30 days)
        let retention_days = db.get_setting("retention_days").await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30);
        if let Err(e) = db.cleanup_old_checks(retention_days).await {
            tracing::warn!("Scheduler: cleanup failed: {}", e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}

async fn run_monitor_check(
    db: &Database,
    monitor: &vigilatrs::models::Monitor,
    notifiers: &std::collections::HashMap<String, vigilatrs::models::Notifier>,
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
    };

    if let Err(e) = db.insert_check(&check).await {
        tracing::error!("Scheduler: failed to save check for {}: {}", monitor.name, e);
        return;
    }

    // Broadcast SSE event
    let event = serde_json::json!({
        "type": "check",
        "monitor_id": monitor.id,
        "monitor_name": monitor.name,
        "status": check.status,
        "response_time_ms": check.response_time_ms,
        "error_message": check.error_message,
        "checked_at": check.checked_at,
    }).to_string();
    let _ = event_tx.send(event);

    // Detect status change and notify
    let is_up = check.status == "up" || check.status == "warning";
    if was_up != is_up {
        if let Some(notifier_id) = &monitor.notifier_id {
            if let Some(notifier) = notifiers.get(notifier_id) {
                if notifier.enabled && notifier.notifier_type == "telegram" {
                    let bot_token = notifier
                        .config_json
                        .get("bot_token")
                        .and_then(|v| v.as_str());
                    let chat_id = notifier
                        .config_json
                        .get("chat_id")
                        .and_then(|v| v.as_str());

                    if let (Some(token), Some(chat)) = (bot_token, chat_id) {
                        if let Err(e) = notifier::send_telegram_notification(
                            token, chat, monitor, &check, was_up,
                        )
                        .await
                        {
                            tracing::warn!(
                                "Scheduler: notification failed for {}: {}",
                                monitor.name,
                                e
                            );
                        }
                    }
                }
            }
        }
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

/// Check if a path is public (no auth required).
fn is_public_path(path: &str) -> bool {
    path == "/"
        || path == "/index.html"
        || path == "/health"
        || path.starts_with("/auth/")
        || path.starts_with("/assets/")
        || path == "/api/events"
        || path.ends_with(".html")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".png")
        || path.ends_with(".ico")
        || path.ends_with(".svg")
        || path.ends_with(".json")
        || path.ends_with(".woff2")
        || path.ends_with(".woff")
        || path.ends_with(".ttf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_root() {
        assert!(is_public_path("/"));
    }

    #[test]
    fn test_is_public_health() {
        assert!(is_public_path("/health"));
    }

    #[test]
    fn test_is_public_auth() {
        assert!(is_public_path("/auth/login"));
        assert!(is_public_path("/auth/callback?code=x"));
    }

    #[test]
    fn test_is_public_assets() {
        assert!(is_public_path("/assets/main.js"));
        assert!(is_public_path("/assets/style.css"));
    }

    #[test]
    fn test_is_public_file_extensions() {
        assert!(is_public_path("/index.html"));
        assert!(is_public_path("/app.js"));
        assert!(is_public_path("/style.css"));
        assert!(is_public_path("/icon.png"));
        assert!(is_public_path("/favicon.ico"));
        assert!(is_public_path("/logo.svg"));
        assert!(is_public_path("/data.json"));
        assert!(is_public_path("/font.woff2"));
        assert!(is_public_path("/font.woff"));
        assert!(is_public_path("/font.ttf"));
    }

    #[test]
    fn test_is_not_public_api() {
        assert!(!is_public_path("/api/status"));
        assert!(!is_public_path("/api/monitors"));
        assert!(!is_public_path("/api/me"));
    }
}