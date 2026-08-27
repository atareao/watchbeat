use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AppState;
use crate::checker;
use crate::models::{CheckResult, Monitor};

// ───── Request types ─────

#[derive(Deserialize)]
pub struct MonitorListParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub q: Option<String>,
    #[serde(rename = "type")]
    pub filter_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateMonitorRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub monitor_type: String,
    pub target: String,
    pub config: Option<serde_json::Value>,
    pub interval_seconds: Option<i64>,
    pub timeout_seconds: Option<i64>,
    pub enabled: Option<bool>,
    pub notifier_id: Option<String>,
    pub confirmations_required: Option<i64>,
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
    pub message_template_expiry: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMonitorRequest {
    pub name: Option<String>,
    pub target: Option<String>,
    pub config: Option<serde_json::Value>,
    pub interval_seconds: Option<i64>,
    pub timeout_seconds: Option<i64>,
    pub enabled: Option<bool>,
    pub notifier_id: Option<String>,
    pub confirmations_required: Option<i64>,
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
    pub message_template_expiry: Option<String>,
}

// ───── Handlers ─────

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let monitor = state
        .db
        .get_monitor(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Monitor not found".to_string())?;

    Ok(Json(serde_json::json!(monitor)))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MonitorListParams>,
) -> Result<Json<serde_json::Value>, String> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let search = params.q.as_deref();
    let filter_type = params.filter_type.as_deref();
    let filter_status = params.status.as_deref();

    let (_monitors, total, summaries) = state
        .db
        .list_monitors_paginated(page, per_page, search, filter_type, filter_status)
        .await
        .map_err(|e| e.to_string())?;

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    // Get global dashboard stats (unfiltered)
    let status = state
        .db
        .get_dashboard_status()
        .await
        .map_err(|e| e.to_string())?;

    let scheduler = state.scheduler_status.lock().await;
    let sched_info = serde_json::json!({
        "last_run_at": scheduler.last_run_at,
        "next_run_at": scheduler.next_run_at,
        "last_monitors_checked": scheduler.last_monitors_checked,
    });

    Ok(Json(serde_json::json!({
        "status": status,
        "monitors": summaries,
        "scheduler": sched_info,
        "total": total,
        "page": page,
        "per_page": per_page,
        "total_pages": total_pages,
    })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateMonitorRequest>,
) -> Result<Json<serde_json::Value>, String> {
    // Check name uniqueness
    if !state
        .db
        .check_name_unique("monitors", "name", &req.name, None)
        .await
        .map_err(|e| e.to_string())?
    {
        return Err(format!("Ya existe un monitor con el nombre '{}'", req.name));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let monitor = Monitor {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        monitor_type: req.monitor_type,
        target: req.target,
        config_json: req.config.unwrap_or(serde_json::json!({})),
        interval_seconds: req.interval_seconds.unwrap_or(300),
        timeout_seconds: req.timeout_seconds.unwrap_or(30),
        enabled: req.enabled.unwrap_or(true),
        notifier_id: req.notifier_id,
        confirmations_required: req.confirmations_required.unwrap_or(0),
        failed_checks: 0,
        latency_threshold_ms: req.latency_threshold_ms,
        message_template_down: req.message_template_down,
        message_template_latency: req.message_template_latency,
        message_template_up: req.message_template_up,
        message_template_expiry: req.message_template_expiry,
        tags: vec![],
        created_at: now.clone(),
        updated_at: now,
    };

    state
        .db
        .create_monitor(&monitor)
        .await
        .map_err(|e| e.to_string())?;

    // Sync the many-to-many monitor_notifiers table
    if let Some(ref nid) = monitor.notifier_id {
        state
            .db
            .set_monitor_notifiers(&monitor.id, std::slice::from_ref(nid))
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(Json(serde_json::json!(monitor)))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMonitorRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let existing = state
        .db
        .get_monitor(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Monitor not found")?;

    // Check name uniqueness if name is being changed
    if let Some(ref name) = req.name {
        if name != &existing.name
            && !state
                .db
                .check_name_unique("monitors", "name", name, Some(&id))
                .await
                .map_err(|e| e.to_string())?
        {
            return Err(format!("Ya existe un monitor con el nombre '{}'", name));
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let monitor = Monitor {
        id: existing.id,
        name: req.name.unwrap_or(existing.name),
        monitor_type: existing.monitor_type,
        target: req.target.unwrap_or(existing.target),
        config_json: req.config.unwrap_or(existing.config_json),
        interval_seconds: req.interval_seconds.unwrap_or(existing.interval_seconds),
        timeout_seconds: req.timeout_seconds.unwrap_or(existing.timeout_seconds),
        enabled: req.enabled.unwrap_or(existing.enabled),
        notifier_id: req.notifier_id.or(existing.notifier_id),
        confirmations_required: req
            .confirmations_required
            .unwrap_or(existing.confirmations_required),
        failed_checks: existing.failed_checks,
        latency_threshold_ms: req.latency_threshold_ms.or(existing.latency_threshold_ms),
        message_template_down: req.message_template_down.or(existing.message_template_down),
        message_template_latency: req
            .message_template_latency
            .or(existing.message_template_latency),
        message_template_up: req.message_template_up.or(existing.message_template_up),
        message_template_expiry: req
            .message_template_expiry
            .or(existing.message_template_expiry),
        tags: existing.tags,
        created_at: existing.created_at,
        updated_at: now,
    };

    state
        .db
        .update_monitor(&id, &monitor)
        .await
        .map_err(|e| e.to_string())?;

    // Sync the many-to-many monitor_notifiers table with the final notifier_id
    if let Some(ref nid) = monitor.notifier_id {
        state
            .db
            .set_monitor_notifiers(&id, std::slice::from_ref(nid))
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Clear the monitor_notifiers table if notifier was removed
        state
            .db
            .set_monitor_notifiers(&id, &[])
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(Json(serde_json::json!(monitor)))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let deleted = state
        .db
        .delete_monitor(&id)
        .await
        .map_err(|e| e.to_string())?;

    if !deleted {
        return Err("Monitor not found".into());
    }
    Ok(Json(serde_json::json!({"deleted": true})))
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let enabled = state
        .db
        .toggle_monitor(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Monitor not found")?;

    Ok(Json(serde_json::json!({"enabled": enabled})))
}

pub async fn run_check(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, String> {
    let monitor = state
        .db
        .get_monitor(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Monitor not found")?;

    let checker = checker::checker_for(&monitor).ok_or("Unsupported monitor type")?;
    let outcome = checker.check(&monitor).await;

    let now = chrono::Utc::now().to_rfc3339();
    let check = CheckResult {
        id: 0,
        monitor_id: id.clone(),
        status: outcome.status,
        status_code: outcome.status_code,
        response_time_ms: outcome.response_time_ms as i64,
        error_message: outcome.error_message,
        checked_at: now,
    };

    let check_id = state
        .db
        .insert_check(&check)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({
                    "id": check_id,
                    "monitor_id": monitor.id,
                    "status": check.status,
                    "status_code": check.status_code,
                    "response_time_ms": check.response_time_ms,
                    "error_message": check.error_message,
                    "checked_at": check.checked_at,
    })))
}
