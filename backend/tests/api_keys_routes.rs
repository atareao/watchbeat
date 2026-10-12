// ───── API keys HTTP handler tests (feature: api-keys, tarea 1.3) ─────
// Ejercita los handlers HTTP reales (`list` / `create` / `update` / `delete`)
// y verifica sus códigos de estado (201/400/404/409). Monta el router real
// (sin el middleware de auth, que se aplica en main.rs) sobre un AppState
// construido con una Database temporal.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use watchbeat::auth::{AppState, JwtValidator, SSE_CHANNEL_CAPACITY};
use watchbeat::config::Config;
use watchbeat::db::Database;
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

/// Envía una request al router montado y devuelve el (status, body).
async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

async fn router(state: Arc<AppState>) -> axum::Router {
    routes::api_routes().with_state(state)
}

// ── create: 201 ──

#[tokio::test]
async fn create_returns_201() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, body) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "ci-cd" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "ci-cd");
    assert!(body["api_key"].is_string());
}

// ── create: 400 name vacío / whitespace ──

#[tokio::test]
async fn create_empty_name_returns_400() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_whitespace_name_returns_400() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── create: 409 duplicate name ──

#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "dup" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "dup" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

// ── update: 404 inexistente ──

#[tokio::test]
async fn update_nonexistent_id_returns_404() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, _) = send(
        &app,
        "PUT",
        "/api/api-keys/no-such-id",
        Some(serde_json::json!({ "name": "renamed" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── update: 409 duplicate (excluyendo id) ──

#[tokio::test]
async fn update_duplicate_name_returns_409() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    // Crear dos keys con nombres distintos.
    let (status, first) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "alpha" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = first["id"].as_str().unwrap().to_string();
    let (status, _) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "beta" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Renombrar `alpha` → `beta` (ocupado) → 409, excluyendo el propio id.
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/api-keys/{}", first_id),
        Some(serde_json::json!({ "name": "beta" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "body: {body}");
}

#[tokio::test]
async fn update_same_name_ok() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, first) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "keep" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = first["id"].as_str().unwrap().to_string();
    // Renombrar a su propio nombre (excluido) → 200.
    let (status, _) = send(
        &app,
        "PUT",
        &format!("/api/api-keys/{}", first_id),
        Some(serde_json::json!({ "name": "keep" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── delete: 404 inexistente / 200 ok ──

#[tokio::test]
async fn delete_nonexistent_id_returns_404() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, _) = send(&app, "DELETE", "/api/api-keys/no-such-id", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_returns_200() {
    let (state, _dir) = setup().await;
    let app = router(state).await;
    let (status, key) = send(
        &app,
        "POST",
        "/api/api-keys",
        Some(serde_json::json!({ "name": "to-delete" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let key_id = key["id"].as_str().unwrap().to_string();
    let (status, _) = send(&app, "DELETE", &format!("/api/api-keys/{}", key_id), None).await;
    assert_eq!(status, StatusCode::OK);
}
