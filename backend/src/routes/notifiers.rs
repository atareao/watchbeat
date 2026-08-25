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
    // Check name uniqueness
    if !state
        .db
        .check_name_unique("notifiers", "name", &req.name, None)
        .await
        .map_err(|e| e.to_string())?
    {
        return Err(format!("Ya existe un notificador con el nombre '{}'", req.name));
    }

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

    // Check name uniqueness if name is being changed
    if let Some(ref name) = req.name {
        if name != &existing.name {
            if !state
                .db
                .check_name_unique("notifiers", "name", name, Some(&id))
                .await
                .map_err(|e| e.to_string())?
            {
                return Err(format!("Ya existe un notificador con el nombre '{}'", name));
            }
        }
    }

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

    let fake_monitor = crate::models::Monitor {
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
    };

    let fake_check = crate::models::CheckResult {
        id: 0,
        monitor_id: "test".into(),
        status: "up".into(),
        status_code: Some(200),
        response_time_ms: 42,
        error_message: None,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };

    match notifier.notifier_type.as_str() {
        "telegram" => {
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
                bot_token, chat_id, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        "matrix" => {
            let homeserver_url = notifier
                .config_json
                .get("homeserver_url")
                .and_then(|v| v.as_str())
                .ok_or("Missing homeserver_url in notifier config")?;
            let access_token = notifier
                .config_json
                .get("access_token")
                .and_then(|v| v.as_str())
                .ok_or("Missing access_token in notifier config")?;
            let room_id = notifier
                .config_json
                .get("room_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing room_id in notifier config")?;
            crate::notifier::matrix::send_matrix_notification(
                homeserver_url, access_token, room_id, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        "ntfy" => {
            let topic = notifier
                .config_json
                .get("topic")
                .and_then(|v| v.as_str())
                .ok_or("Missing topic in notifier config")?;
            let server_url = notifier
                .config_json
                .get("server_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://ntfy.sh");
            let token = notifier.config_json.get("token").and_then(|v| v.as_str());
            crate::notifier::ntfy::send_ntfy_notification(
                topic, server_url, token, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        "webhook" => {
            let url = notifier
                .config_json
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("Missing url in notifier config")?;
            let method = notifier
                .config_json
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("POST");
            let headers_json = notifier
                .config_json
                .get("headers")
                .map(|v| v.to_string())
                .unwrap_or_default();
            crate::notifier::webhook::send_webhook_notification(
                url, method, &headers_json, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        "slack" => {
            let webhook_url = notifier
                .config_json
                .get("webhook_url")
                .and_then(|v| v.as_str())
                .ok_or("Missing webhook_url in notifier config")?;
            crate::notifier::slack::send_slack_notification(webhook_url, &fake_monitor, &fake_check, false)
                .await
                .map_err(|e| e.to_string())?;
        }
        "discord" => {
            let webhook_url = notifier
                .config_json
                .get("webhook_url")
                .and_then(|v| v.as_str())
                .ok_or("Missing webhook_url in notifier config")?;
            crate::notifier::discord::send_discord_notification(webhook_url, &fake_monitor, &fake_check, false)
                .await
                .map_err(|e| e.to_string())?;
        }
        "email" => {
            let smtp_host = notifier
                .config_json
                .get("smtp_host")
                .and_then(|v| v.as_str())
                .ok_or("Missing smtp_host in notifier config")?;
            let smtp_port = notifier
                .config_json
                .get("smtp_port")
                .and_then(|v| v.as_u64())
                .unwrap_or(587) as u16;
            let username = notifier
                .config_json
                .get("username")
                .and_then(|v| v.as_str())
                .ok_or("Missing username in notifier config")?;
            let password = notifier
                .config_json
                .get("password")
                .and_then(|v| v.as_str())
                .ok_or("Missing password in notifier config")?;
            let from = notifier
                .config_json
                .get("from")
                .and_then(|v| v.as_str())
                .ok_or("Missing from in notifier config")?;
            let to = notifier
                .config_json
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or("Missing to in notifier config")?;
            crate::notifier::email::send_email_notification(
                smtp_host, smtp_port, username, password, from, to, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        "gotify" => {
            let server_url = notifier
                .config_json
                .get("server_url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080");
            let app_token = notifier
                .config_json
                .get("app_token")
                .and_then(|v| v.as_str())
                .ok_or("Missing app_token in notifier config")?;
            let priority = notifier
                .config_json
                .get("priority")
                .and_then(|v| v.as_i64())
                .unwrap_or(5);
            crate::notifier::gotify::send_gotify_notification(
                server_url, app_token, priority, &fake_monitor, &fake_check, false,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("Unsupported notifier type: {}", notifier.notifier_type)),
    }

    Ok(Json(serde_json::json!({"sent": true})))
}