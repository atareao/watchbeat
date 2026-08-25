use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing;
use axum::Router;
use futures::stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::AppState;

pub mod auth_routes;
pub mod backup;
pub mod checks;
pub mod exports;
pub mod heartbeats;
pub mod metrics;
pub mod monitors;
pub mod notifiers;
pub mod settings;
pub mod status;
pub mod status_pages;

pub fn api_routes() -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/health", routing::get(health))
        .route("/auth/login", routing::get(auth_routes::login))
        .route("/auth/callback", routing::get(auth_routes::callback))
        .route("/auth/logout", routing::get(auth_routes::logout))
        .route("/status/{slug}", routing::get(status_pages::public_page))
        .route("/metrics", routing::get(metrics::metrics_handler))
        .route("/api/heartbeat/{token}", routing::post(heartbeats::ping));

    let events_route = Router::new().route("/api/events", routing::get(sse_handler));

    let protected = Router::new()
        .route("/api/me", routing::get(auth_routes::me))
        .route(
            "/api/monitors",
            routing::get(monitors::list).post(monitors::create),
        )
        .route(
            "/api/monitors/{id}",
            routing::put(monitors::update)
                .delete(monitors::delete)
                .patch(monitors::toggle),
        )
        .route(
            "/api/monitors/{id}/check",
            routing::post(monitors::run_check),
        )
        .route("/api/monitors/{id}/checks", routing::get(checks::list))
        .route(
            "/api/monitors/{id}/timeline",
            routing::get(checks::timeline),
        )
        .route("/api/checks/recent", routing::get(checks::recent_global))
        .route(
            "/api/notifiers",
            routing::get(notifiers::list).post(notifiers::create),
        )
        .route(
            "/api/notifiers/{id}",
            routing::put(notifiers::update).delete(notifiers::delete),
        )
        .route("/api/notifiers/{id}/test", routing::post(notifiers::test))
        .route(
            "/api/status-pages",
            routing::get(status_pages::list).post(status_pages::create),
        )
        .route(
            "/api/status-pages/{id}",
            routing::put(status_pages::update).delete(status_pages::delete),
        )
        .route(
            "/api/heartbeats",
            routing::get(heartbeats::list).post(heartbeats::create),
        )
        .route(
            "/api/heartbeats/{id}",
            routing::put(heartbeats::update).delete(heartbeats::delete),
        )
        .route(
            "/api/monitors/{id}/export/{format}",
            routing::get(exports::export),
        )
        .route("/api/backup", routing::post(backup::create_backup))
        .route(
            "/api/settings",
            routing::get(settings::get_setting_query).post(settings::set_setting),
        )
        .route("/api/status", routing::get(status::dashboard));

    public.merge(events_route).merge(protected)
}

pub async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
}

/// SSE endpoint — streams check events in real-time.
/// Validates token via query param (EventSource API) or Authorization header.
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, String> {
    // Validate token: query param (EventSource API friendly) or Bearer header
    let token = req.uri().query().and_then(|q| {
        q.split('&').find_map(|p| {
            let mut parts = p.splitn(2, '=');
            if parts.next()? == "token" {
                parts.next()
            } else {
                None
            }
        })
    });

    let token = match token {
        Some(t) => t.to_string(),
        None => {
            // Try Authorization header
            let auth = req
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string());

            match auth {
                Some(t) => t,
                None => {
                    return Err(
                        "Missing authentication. Use ?token= or Authorization: Bearer".into(),
                    )
                }
            }
        }
    };

    if state.jwt_validator.validate_token(&token).await.is_err() {
        return Err("Invalid token".into());
    }

    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| match msg {
        Ok(data) => Ok(Event::default().data(data)),
        Err(_) => Ok(Event::default().data("")),
    });

    Ok(Sse::new(stream))
}
