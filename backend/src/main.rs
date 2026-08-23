use std::sync::Arc;

use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use vigilatrs::auth::{self, AppState, JwtValidator, OidcState, SchedulerStatus};
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

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db: db.clone(),
        oidc_metadata: Some(oidc_metadata),
        jwt_validator: jwt_validator.clone(),
        oidc_states: Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        scheduler_status: scheduler_status.clone(),
    });

    // ───── Scheduler ─────
    let db_for_scheduler = db.clone();
    let sched_status = scheduler_status.clone();
    tokio::spawn(async move {
        scheduler_loop(db_for_scheduler, sched_status).await;
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
                    run_monitor_check(&db, monitor, &notifiers).await;
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
                    run_monitor_check(&db, monitor, &notifiers).await;
                    checks_done += 1;
                    continue;
                }
            };

            let elapsed = chrono::Utc::now() - last_time;
            let interval = chrono::Duration::seconds(monitor.interval_seconds);

            if elapsed >= interval {
                run_monitor_check(&db, monitor, &notifiers).await;
                checks_done += 1;
            }
        }

        // Update scheduler status
        {
            let mut status = sched_status.lock().await;
            status.last_run_at = Some(chrono::Utc::now().to_rfc3339());
            status.last_monitors_checked = monitors.len() as u64;
        }

        // Cleanup old checks (keep 30 days)
        if let Err(e) = db.cleanup_old_checks(30).await {
            tracing::warn!("Scheduler: cleanup failed: {}", e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}

async fn run_monitor_check(
    db: &Database,
    monitor: &vigilatrs::models::Monitor,
    notifiers: &std::collections::HashMap<String, vigilatrs::models::Notifier>,
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
    let check = CheckResult {
        id: 0,
        monitor_id: monitor.id.clone(),
        status: outcome.status,
        status_code: outcome.status_code,
        response_time_ms: outcome.response_time_ms,
        error_message: outcome.error_message,
        checked_at: now_str,
    };

    if let Err(e) = db.insert_check(&check).await {
        tracing::error!("Scheduler: failed to save check for {}: {}", monitor.name, e);
        return;
    }

    // Detect status change and notify
    let is_up = check.status == "up";
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