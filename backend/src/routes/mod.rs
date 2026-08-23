use std::sync::Arc;

use axum::routing;
use axum::Router;

use crate::auth::AppState;

pub mod auth_routes;
pub mod checks;
pub mod monitors;
pub mod notifiers;
pub mod status;

pub fn api_routes() -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/health", routing::get(health))
        .route("/auth/login", routing::get(auth_routes::login))
        .route("/auth/callback", routing::get(auth_routes::callback));

    let protected = Router::new()
        .route("/api/me", routing::get(auth_routes::me))
        .route("/api/monitors", routing::get(monitors::list).post(monitors::create))
        .route(
            "/api/monitors/{id}",
            routing::put(monitors::update)
                .delete(monitors::delete)
                .patch(monitors::toggle),
        )
        .route("/api/monitors/{id}/check", routing::post(monitors::run_check))
        .route("/api/monitors/{id}/checks", routing::get(checks::list))
        .route("/api/monitors/{id}/timeline", routing::get(checks::timeline))
        .route("/api/checks/recent", routing::get(checks::recent_global))
        .route("/api/notifiers", routing::get(notifiers::list).post(notifiers::create))
        .route(
            "/api/notifiers/{id}",
            routing::put(notifiers::update)
                .delete(notifiers::delete),
        )
        .route(
            "/api/notifiers/{id}/test",
            routing::post(notifiers::test),
        )
        .route("/api/status", routing::get(status::dashboard));

    public.merge(protected)
}

pub async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
}