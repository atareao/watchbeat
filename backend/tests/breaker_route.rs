// ───── GET /api/breaker route tests (feature: breaker-state) ─────
// Ejercita el handler real `GET /api/breaker` y fija el contrato de la spec:
//   (a) 200 con `state:"closed"` y la configuración vigente del breaker
//   (b) `state:"open"` cuando el breaker compartido se abre por una tormenta
//   (c) 401 sin token (el endpoint NO es público)
//
// Igual que el resto de tests de rutas del repo, (a) y (b) montan el router
// real (`routes::api_routes()`) sin el middleware de auth — ese se aplica en
// `main.rs` — para ejercitar el handler. El caso (c) monta el router **con** el
// middleware `require_auth`, que es lo que fija que `/api/breaker` no esté en la
// lista blanca de rutas públicas.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use watchbeat::auth::{AppState, JwtValidator, SSE_CHANNEL_CAPACITY};
use watchbeat::breaker::BreakerState;
use watchbeat::config::Config;
use watchbeat::db::Database;
use watchbeat::routes;
use watchbeat::scheduler::SchedulerManager;

async fn setup() -> (Arc<AppState>, TempDir) {
    setup_with_settings(&[]).await
}

/// `setup` con settings precargadas en la BD **antes** de arrancar el scheduler,
/// para poder ejercitar `BreakerConfig::load → breaker → API` con valores no-default.
async fn setup_with_settings(settings: &[(&str, &str)]) -> (Arc<AppState>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = Database::open(&path).await.unwrap();

    for (key, value) in settings {
        db.set_setting(key, value).await.unwrap();
    }

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
    let netcheck_handle = watchbeat::netcheck::NetworkHealthHandle::new();
    let scheduler_mgr =
        SchedulerManager::spawn(db.clone(), event_tx.clone(), netcheck_handle.clone()).await;

    let state = Arc::new(AppState {
        config,
        db,
        oidc_metadata: None,
        jwt_validator: Arc::new(JwtValidator::new("issuer", "client")),
        oidc_states: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        scheduler_mgr,
        event_tx,
        netcheck: netcheck_handle,
    });
    (state, dir)
}

/// Router real sin el middleware de auth (se aplica en `main.rs`).
fn app_without_auth(state: Arc<AppState>) -> axum::Router {
    routes::api_routes().with_state(state)
}

/// Router con el middleware `require_auth` cableado de la misma forma que
/// `main.rs` (inyecta el `AppState` en las extensiones antes de validar).
fn app_with_auth(state: Arc<AppState>) -> axum::Router {
    let state_for_middleware = state.clone();
    routes::api_routes()
        .layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let state = state_for_middleware.clone();
                async move {
                    req.extensions_mut().insert(state);
                    watchbeat::auth::require_auth(req, next).await
                }
            },
        ))
        .with_state(state)
}

/// GET /api/breaker y devuelve el (status, body) parseado.
async fn get_breaker(app: &axum::Router) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("GET")
        .uri("/api/breaker")
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

/// Escenario: estado cerrado en condiciones normales.
/// Un breaker recién creado está `closed` y expone la configuración vigente: sin
/// settings `storm_*` en la BD, los defaults de [`BreakerConfig::default`].
#[tokio::test]
async fn breaker_endpoint_reports_closed_state_and_default_config() {
    let (state, _dir) = setup().await;
    let app = app_without_auth(state);

    let (status, body) = get_breaker(&app).await;

    assert_eq!(status, StatusCode::OK, "GET /api/breaker debe ser 200");
    let status_obj = &body["status"];
    assert_eq!(
        status_obj["state"], "closed",
        "un breaker sin tormenta debe reportar 'closed'"
    );
    // Defaults de `BreakerConfig` (enabled, 50 %, 10 s, 3 monitores por defecto).
    assert_eq!(status_obj["enabled"], true, "enabled por defecto");
    assert_eq!(
        status_obj["threshold_percent"], 50,
        "threshold_percent por defecto"
    );
    assert_eq!(
        status_obj["window_seconds"], 10,
        "window_seconds por defecto"
    );
    assert_eq!(status_obj["min_monitors"], 3, "min_monitors por defecto");
}

/// Escenario: la API refleja la configuración vigente, no los defaults.
/// Siembra settings `storm_*` no-default, dispara la recarga en caliente
/// (`ReloadBreaker`) y comprueba que `GET /api/breaker` las refleja: ejercita el
/// tramo `BreakerConfig::load → breaker compartido → API`.
#[tokio::test]
async fn breaker_endpoint_reflects_settings_after_hot_reload() {
    let (state, _dir) = setup().await;

    // La recarga relee los settings de la BD (D6).
    state
        .db
        .set_setting("storm_breaker_enabled", "true")
        .await
        .unwrap();
    state
        .db
        .set_setting("storm_threshold_percent", "80")
        .await
        .unwrap();
    state
        .db
        .set_setting("storm_window_seconds", "45")
        .await
        .unwrap();
    state
        .db
        .set_setting("storm_min_monitors", "7")
        .await
        .unwrap();

    state.scheduler_mgr.reload_breaker().await;

    // El comando es asíncrono y sin ack: espera a que el breaker compartido lo
    // haya aplicado (con timeout, para no colgar el test si no llega nunca).
    let breaker = state.scheduler_mgr.breaker();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if breaker.lock().await.config().min_monitors == 7 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("el breaker compartido debe aplicar la config recargada");

    let app = app_without_auth(state);
    let (status, body) = get_breaker(&app).await;

    assert_eq!(status, StatusCode::OK, "GET /api/breaker debe ser 200");
    let status_obj = &body["status"];
    assert_eq!(status_obj["state"], "closed", "sigue sin tormenta");
    assert_eq!(
        status_obj["threshold_percent"], 80,
        "threshold_percent recargado desde settings"
    );
    assert_eq!(
        status_obj["window_seconds"], 45,
        "window_seconds recargado desde settings"
    );
    assert_eq!(
        status_obj["min_monitors"], 7,
        "min_monitors recargado desde settings"
    );
    assert_eq!(
        status_obj["enabled"], true,
        "enabled recargado desde settings"
    );
}

/// Escenario: estado abierto durante una tormenta.
/// El endpoint debe leer el breaker **real y compartido** con el scheduler.
#[tokio::test]
async fn breaker_endpoint_reports_open_state_after_a_storm() {
    let (state, _dir) = setup().await;

    // Fuerza una tormenta en el breaker compartido: con la config por defecto
    // (min_monitors = 3) tres caídas distintas abren el breaker.
    {
        let breaker = state.scheduler_mgr.breaker();
        let mut guard = breaker.lock().await;
        assert_eq!(
            guard.state(),
            BreakerState::Closed,
            "el breaker compartido debe empezar cerrado"
        );
        let now = Instant::now();
        for id in ["m-1", "m-2", "m-3"] {
            guard.on_down(now, id, id, 3);
        }
    }

    let app = app_without_auth(state);
    let (status, body) = get_breaker(&app).await;

    assert_eq!(status, StatusCode::OK, "GET /api/breaker debe ser 200");
    assert_eq!(
        body["status"]["state"], "open",
        "tras la tormenta el endpoint debe reportar 'open' desde el breaker compartido"
    );
}

/// Escenario: el endpoint requiere autenticación.
#[tokio::test]
async fn breaker_endpoint_requires_authentication() {
    let (state, _dir) = setup().await;
    let app = app_with_auth(state);

    let (status, _body) = get_breaker(&app).await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "/api/breaker no está en la lista blanca de rutas públicas: sin token debe ser 401"
    );
}
