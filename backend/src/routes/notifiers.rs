use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::models::Notifier;

#[derive(Deserialize)]
pub struct CreateNotifierRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub notifier_type: String,
    pub config: serde_json::Value,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateNotifierRequest {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, String> {
    let notifiers = state.db.list_notifiers().await.map_err(|e| e.to_string())?;
    Ok(Json(serde_json::json!({ "notifiers": notifiers })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateNotifierRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let notifier = Notifier {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        notifier_type: req.notifier_type,
        config_json: req.config,
        enabled: req.enabled.unwrap_or(true),
        created_at: now.clone(),
        updated_at: now,
    };

    state
        .db
        .upsert_notifier(&notifier.id, &notifier)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(notifier)))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNotifierRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let existing = state
        .db
        .get_notifier(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Notifier not found")?;

    let now = chrono::Utc::now().to_rfc3339();
    let notifier = Notifier {
        id: existing.id,
        name: req.name.unwrap_or(existing.name),
        notifier_type: existing.notifier_type,
        config_json: req.config.unwrap_or(existing.config_json),
        enabled: req.enabled.unwrap_or(existing.enabled),
        created_at: existing.created_at,
        updated_at: now,
    };

    state
        .db
        .upsert_notifier(&id, &notifier)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!(notifier)))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let deleted = state
        .db
        .delete_notifier(&id)
        .await
        .map_err(|e| e.to_string())?;

    if !deleted {
        return Err("Notifier not found".into());
    }
    Ok(Json(serde_json::json!({"deleted": true})))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let notifier = state
        .db
        .get_notifier(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Notifier not found")?;

    if notifier.notifier_type != "telegram" {
        return Err("Only Telegram notifiers are supported for testing".into());
    }

    let bot_token = notifier
        .config_json
        .get("bot_token")
        .and_then(|v| v.as_str())
        .ok_or("Missing bot_token in notifier config")?;

    let chat_id = notifier
        .config_json
        .get("chat_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing chat_id in notifier config")?;

    crate::notifier::telegram::send_telegram_notification(
        bot_token,
        chat_id,
        &crate::models::Monitor {
            id: "test".into(),
            name: "Test Notification".into(),
            monitor_type: "http".into(),
            target: "https://example.com".into(),
            config_json: serde_json::json!({}),
            interval_seconds: 300,
            timeout_seconds: 30,
            enabled: true,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            tags: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        },
        &crate::models::CheckResult {
            id: 0,
            monitor_id: "test".into(),
            status: "up".into(),
            status_code: Some(200),
            response_time_ms: 42,
            error_message: None,
            checked_at: chrono::Utc::now().to_rfc3339(),
        },
        false,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({"sent": true})))
}
