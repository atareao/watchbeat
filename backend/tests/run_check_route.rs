// ───── run-check route tests (feature: failure-cause-taxonomy, Mn2) ─────
// Ejercita el handler real `POST /api/monitors/{id}/check` y verifica que la
// respuesta expone `error_kind` cuando el check falla con causa, y que lo omite
// (contrato aditivo) cuando el check es correcto.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use watchbeat::auth::{AppState, JwtValidator, SSE_CHANNEL_CAPACITY};
use watchbeat::config::Config;
use watchbeat::db::Database;
use watchbeat::models::Monitor;
use watchbeat::routes;
use watchbeat::scheduler::SchedulerManager;

async fn setup() -> (Arc<AppState>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = Database::open(&path).await.unwrap();

    let config = Config {
        host: "0.0.0.0".into(),
        port: 3055,
        data_dir: dir.path().into(),
        database_url: path,
        timezone: "Europe/Madrid".into(),
        log_level: "info".into(),
        log_format: "pretty".into(),
        oidc_issuer_url: "https://issuer.example".into(),
        oidc_client_id: "client".into(),
        oidc_client_secret: "secret".into(),
        oidc_redirect_url: "http://localhost:3055/auth/callback".into(),
        retention_days: 30,
        dns_upstreams: watchbeat::dns::default_upstreams(),
    };

    let (event_tx, _) = tokio::sync::broadcast::channel(SSE_CHANNEL_CAPACITY);
    let scheduler_mgr = SchedulerManager::spawn(db.clone(), event_tx.clone()).await;

    let state = Arc::new(AppState {
        config,
        db,
        oidc_metadata: None,
        jwt_validator: Arc::new(JwtValidator::new("issuer", "client")),
        oidc_states: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        scheduler_mgr,
        event_tx,
    });
    (state, dir)
}

fn sample_monitor(id: &str, target: &str) -> Monitor {
    Monitor {
        id: id.into(),
        name: format!("Monitor {id}"),
        monitor_type: "tcp".into(),
        target: target.into(),
        config_json: serde_json::json!({}),
        interval_seconds: 300,
        timeout_seconds: 5,
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

/// A loopback port with nothing listening on it (bound and released).
async fn closed_loopback_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// POST /api/monitors/{id}/check and return the parsed body.
async fn run_check(app: &axum::Router, id: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/monitors/{id}/check"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

/// A failing check must expose its cause in the response.
#[tokio::test]
async fn run_check_returns_error_kind_when_the_check_fails() {
    let (state, _dir) = setup().await;
    let port = closed_loopback_port().await;
    state
        .db
        .create_monitor(&sample_monitor("m-refused", &format!("127.0.0.1:{port}")))
        .await
        .unwrap();

    let app = routes::api_routes().with_state(state);
    let (status, body) = run_check(&app, "m-refused").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "down");
    assert_eq!(body["error_kind"], "connect_refused");
}

/// Additive contract: a healthy check keeps the old payload shape.
#[tokio::test]
async fn run_check_omits_error_kind_when_the_check_is_up() {
    let (state, _dir) = setup().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
    });

    state
        .db
        .create_monitor(&sample_monitor("m-up", &addr.to_string()))
        .await
        .unwrap();

    let app = routes::api_routes().with_state(state);
    let (status, body) = run_check(&app, "m-up").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "up");
    assert!(
        body.get("error_kind").is_none(),
        "a check without cause must not add the field, got {body}"
    );
}
