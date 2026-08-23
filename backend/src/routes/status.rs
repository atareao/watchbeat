use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::auth::AppState;

pub async fn dashboard(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, String> {
    let status = state
        .db
        .get_dashboard_status()
        .await
        .map_err(|e| e.to_string())?;

    let summaries = state
        .db
        .get_monitor_summaries()
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
    })))
}