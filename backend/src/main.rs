use std::sync::Arc;

use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use watchbeat::auth::{self, AppState, JwtValidator};
use watchbeat::config::Config;
use watchbeat::db::Database;
use watchbeat::embed::serve_embedded;
use watchbeat::routes;
use watchbeat::scheduler::SchedulerManager;

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
    let (event_tx, _) = tokio::sync::broadcast::channel(auth::SSE_CHANNEL_CAPACITY);

    // Start the scheduler (per-monitor timers, no more polling)
    let scheduler_mgr = SchedulerManager::spawn(db.clone(), event_tx.clone()).await;

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db: db.clone(),
        oidc_metadata: Some(oidc_metadata),
        jwt_validator: jwt_validator.clone(),
        oidc_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        scheduler_mgr,
        event_tx: event_tx.clone(),
    });

    // ───── Daily Cleanup Loop ─────
    let db_for_cleanup = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
        // First run after 1h to let scheduler populate data
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        loop {
            interval.tick().await;
            tracing::info!(
                "Running daily cleanup of old checks (retention={}d)",
                config.retention_days
            );
            if let Err(e) = db_for_cleanup
                .cleanup_old_checks(config.retention_days)
                .await
            {
                tracing::warn!("Cleanup: failed: {}", e);
            }
        }
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
