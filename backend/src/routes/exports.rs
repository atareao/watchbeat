use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::response::Response;

use crate::auth::AppState;

pub async fn export(
    State(state): State<Arc<AppState>>,
    Path((id, format)): Path<(String, String)>,
) -> Result<Response, String> {
    let checks = state
        .db
        .get_checks(&id, 10000, 0)
        .await
        .map_err(|e| e.to_string())?;

    match format.as_str() {
        "csv" => {
            let mut csv =
                String::from("checked_at,status,status_code,response_time_ms,error_message\n");
            for c in &checks {
                let err = c.error_message.as_deref().unwrap_or("");
                let code = c.status_code.map(|v| v.to_string()).unwrap_or_default();
                csv.push_str(&format!(
                    "{},{},{},{},{}\n",
                    c.checked_at, c.status, code, c.response_time_ms, err
                ));
            }
            Ok(Response::builder()
                .header("Content-Type", "text/csv; charset=utf-8")
                .header(
                    "Content-Disposition",
                    &format!("attachment; filename=\"checks-{}.csv\"", id),
                )
                .body(Body::from(csv))
                .unwrap())
        }
        "json" => {
            let json = serde_json::to_string_pretty(&checks).map_err(|e| e.to_string())?;
            Ok(Response::builder()
                .header("Content-Type", "application/json; charset=utf-8")
                .header(
                    "Content-Disposition",
                    &format!("attachment; filename=\"checks-{}.json\"", id),
                )
                .body(Body::from(json))
                .unwrap())
        }
        _ => Err("Unsupported format. Use 'csv' or 'json'.".into()),
    }
}
