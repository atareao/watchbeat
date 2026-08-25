use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::auth::AppState;

pub async fn create_backup(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, String> {
    let output_path = state.config.data_dir.join("watchbeat.backup.db");

    state
        .db
        .backup(&output_path)
        .await
        .map_err(|e| format!("Backup failed: {}", e))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "path": output_path.to_string_lossy(),
    })))
}
