use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::AppState;

#[derive(Serialize, Deserialize, Clone)]
pub struct ExportPayload {
    pub version: String,
    pub exported_at: String,
    pub monitors: Vec<serde_json::Value>,
    pub notifiers: Vec<serde_json::Value>,
    pub status_pages: Vec<serde_json::Value>,
    pub settings: Vec<serde_json::Value>,
}

/// GET /api/export — export all configuration as JSON
pub async fn export_all(State(state): State<Arc<AppState>>) -> Result<Json<ExportPayload>, String> {
    let monitors = state
        .db
        .list_monitors()
        .await
        .map_err(|e| format!("Failed to list monitors: {}", e))?;

    let notifiers = state
        .db
        .list_notifiers()
        .await
        .map_err(|e| format!("Failed to list notifiers: {}", e))?;

    let status_pages = state
        .db
        .list_status_pages()
        .await
        .map_err(|e| format!("Failed to list status pages: {}", e))?;

    let settings = state
        .db
        .get_all_settings()
        .await
        .map_err(|e| format!("Failed to list settings: {}", e))?;

    let now = chrono::Utc::now().to_rfc3339();

    Ok(Json(ExportPayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: now,
        monitors: monitors
            .into_iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect(),
        notifiers: notifiers
            .into_iter()
            .map(|n| serde_json::to_value(n).unwrap())
            .collect(),
        status_pages: status_pages
            .into_iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect(),
        settings: settings
            .into_iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect(),
    }))
}

/// POST /api/import — import configuration from JSON payload
pub async fn import_all(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExportPayload>,
) -> Result<Json<serde_json::Value>, String> {
    // Import notifiers first (monitors may reference them)
    for notifier in &payload.notifiers {
        let id = notifier
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Notifier missing id")?;
        let name = notifier
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Notifier missing name")?;
        let notifier_type = notifier
            .get("notifier_type")
            .or_else(|| notifier.get("type"))
            .and_then(|v| v.as_str())
            .ok_or("Notifier missing notifier_type")?;
        let config_json = notifier
            .get("config_json")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let enabled = notifier
            .get("enabled")
            .and_then(|v| v.as_i64().or_else(|| v.as_bool().map(|b| b as i64)))
            .unwrap_or(1);

        let existing = state
            .db
            .get_notifier(id)
            .await
            .map_err(|e| format!("Error checking notifier: {}", e))?;
        if existing.is_some() {
            state
                .db
                .update_notifier(id, name, notifier_type, &config_json, enabled != 0)
                .await
                .map_err(|e| format!("Failed to update notifier {}: {}", id, e))?;
        } else {
            state
                .db
                .create_notifier(id, name, notifier_type, &config_json, enabled != 0)
                .await
                .map_err(|e| format!("Failed to create notifier {}: {}", id, e))?;
        }
    }

    // Import monitors
    for monitor in &payload.monitors {
        let id = monitor
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Monitor missing id")?;
        let name = monitor
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Monitor missing name")?;
        let monitor_type = monitor
            .get("monitor_type")
            .or_else(|| monitor.get("type"))
            .and_then(|v| v.as_str())
            .ok_or("Monitor missing type")?;
        let target = monitor.get("target").and_then(|v| v.as_str()).unwrap_or("");
        let config_json = monitor
            .get("config_json")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let interval_seconds = monitor
            .get("interval_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(300)
            .max(60);
        let timeout_seconds = monitor
            .get("timeout_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(30);
        let enabled = monitor
            .get("enabled")
            .and_then(|v| v.as_i64().or_else(|| v.as_bool().map(|b| b as i64)))
            .unwrap_or(1);
        let notifier_id = monitor
            .get("notifier_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let confirmations_required = monitor
            .get("confirmations_required")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let latency_threshold_ms = monitor.get("latency_threshold_ms").and_then(|v| v.as_i64());
        let message_template_down = monitor
            .get("message_template_down")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let message_template_latency = monitor
            .get("message_template_latency")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let message_template_up = monitor
            .get("message_template_up")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let message_template_expiry = monitor
            .get("message_template_expiry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tags: Vec<String> = monitor
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let token = monitor
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let grace_seconds = monitor.get("grace_seconds").and_then(|v| v.as_i64());
        let last_seen_at = monitor
            .get("last_seen_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let existing = state
            .db
            .get_monitor(id)
            .await
            .map_err(|e| format!("Error checking monitor: {}", e))?;
        if existing.is_some() {
            state
                .db
                .update_monitor_fieldwise(
                    id,
                    name,
                    monitor_type,
                    target,
                    &config_json,
                    interval_seconds,
                    timeout_seconds,
                    enabled != 0,
                    notifier_id,
                    confirmations_required,
                    latency_threshold_ms,
                    message_template_down,
                    message_template_latency,
                    message_template_up,
                    message_template_expiry,
                    &tags,
                    token,
                    grace_seconds,
                    last_seen_at,
                )
                .await
                .map_err(|e| format!("Failed to update monitor {}: {}", id, e))?;
        } else {
            state
                .db
                .create_monitor_full(
                    id,
                    name,
                    monitor_type,
                    target,
                    &config_json,
                    interval_seconds,
                    timeout_seconds,
                    enabled != 0,
                    notifier_id,
                    confirmations_required,
                    latency_threshold_ms,
                    message_template_down,
                    message_template_latency,
                    message_template_up,
                    message_template_expiry,
                    &tags,
                    token,
                    grace_seconds,
                    last_seen_at,
                )
                .await
                .map_err(|e| format!("Failed to create monitor {}: {}", id, e))?;
        }
    }

    // Import status pages
    for page in &payload.status_pages {
        let id = page
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Status page missing id")?;
        let slug = page
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or("Status page missing slug")?;
        let title = page
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Status page missing title")?;
        let description = page
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let monitors: Vec<String> = page
            .get("monitors")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let public = page
            .get("public")
            .and_then(|v| v.as_i64().or_else(|| v.as_bool().map(|b| b as i64)))
            .unwrap_or(1);

        let existing = state
            .db
            .get_status_page(id)
            .await
            .map_err(|e| format!("Error checking status page: {}", e))?;
        if existing.is_some() {
            state
                .db
                .update_status_page(
                    id,
                    slug,
                    title,
                    description.as_deref(),
                    &monitors,
                    public != 0,
                )
                .await
                .map_err(|e| format!("Failed to update status page {}: {}", id, e))?;
        } else {
            state
                .db
                .create_status_page(
                    id,
                    slug,
                    title,
                    description.as_deref(),
                    &monitors,
                    public != 0,
                )
                .await
                .map_err(|e| format!("Failed to create status page {}: {}", id, e))?;
        }
    }

    // Import settings
    for setting in &payload.settings {
        let key = setting
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or("Setting missing key")?;
        let value = setting.get("value").and_then(|v| v.as_str()).unwrap_or("");
        state
            .db
            .set_setting(key, value)
            .await
            .map_err(|e| format!("Failed to set setting {}: {}", key, e))?;
    }

    Ok(Json(serde_json::json!({ "ok": true, "imported": {
        "monitors": payload.monitors.len(),
        "notifiers": payload.notifiers.len(),
        "status_pages": payload.status_pages.len(),
        "settings": payload.settings.len(),
    }})))
}
