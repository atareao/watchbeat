use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::models::Heartbeat;

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub name: String,
    pub grace_seconds: Option<i64>,
    pub notifier_id: Option<String>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, String> {
    let hbs = state
        .db
        .list_heartbeats()
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(serde_json::json!({ "heartbeats": hbs })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let token = Uuid::new_v4().to_string().replace('-', "");
    let hb = Heartbeat {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        token: token.clone(),
        grace_seconds: req.grace_seconds.unwrap_or(3600),
        last_seen_at: None,
        status: "pending".into(),
        notifier_id: req.notifier_id,
        created_at: now.clone(),
        updated_at: now,
    };

    state
        .db
        .upsert_heartbeat(&hb.id, &hb)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(hb)))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let existing = state
        .db
        .get_heartbeat(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Heartbeat not found")?;

    let now = chrono::Utc::now().to_rfc3339();
    let hb = Heartbeat {
        id: existing.id,
        name: req.name,
        token: existing.token,
        grace_seconds: req.grace_seconds.unwrap_or(existing.grace_seconds),
        last_seen_at: existing.last_seen_at,
        status: existing.status,
        notifier_id: req.notifier_id.or(existing.notifier_id),
        created_at: existing.created_at,
        updated_at: now,
    };

    state
        .db
        .upsert_heartbeat(&id, &hb)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(hb)))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let deleted = state
        .db
        .delete_heartbeat(&id)
        .await
        .map_err(|e| e.to_string())?;
    if !deleted {
        return Err("Heartbeat not found".into());
    }
    Ok(Json(serde_json::json!({"deleted": true})))
}

// ───── Public heartbeat ping (no auth) ─────

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    match state.db.touch_heartbeat(&token).await {
        Ok(Some(hb)) => Ok(Json(serde_json::json!({
            "ok": true,
            "name": hb.name,
            "last_seen_at": hb.last_seen_at,
        }))),
        Ok(None) => Err("Invalid heartbeat token".into()),
        Err(e) => Err(e.to_string()),
    }
}
