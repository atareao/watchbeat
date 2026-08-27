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

    Ok(Json(serde_json::json!({
        "status": status,
    })))
}
