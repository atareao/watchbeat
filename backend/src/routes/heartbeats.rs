use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use crate::auth::AppState;

// ───── Public heartbeat ping (no auth) ─────

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    match state.db.record_heartbeat_pulse(&token).await {
        Ok(Some(monitor)) => Ok(Json(serde_json::json!({
            "ok": true,
            "name": monitor.name,
            "last_seen_at": monitor.last_seen_at,
        }))),
        Ok(None) => Err("Invalid heartbeat token".into()),
        Err(e) => Err(e.to_string()),
    }
}
