use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::auth::AppState;

#[derive(Deserialize)]
pub struct GetSettingQuery {
    pub key: Option<String>,
}

#[derive(Deserialize)]
pub struct SetSettingBody {
    pub key: String,
    pub value: String,
}

/// Settings that only take effect once the scheduler reloads the storm breaker
/// (Fase 5, D6). Without a hot reload they would need a process restart.
fn is_breaker_setting(key: &str) -> bool {
    key.starts_with("storm_") || key == "default_template_storm"
}

pub async fn set_setting(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetSettingBody>,
) -> Result<Json<serde_json::Value>, String> {
    state
        .db
        .set_setting(&body.key, &body.value)
        .await
        .map_err(|e| e.to_string())?;

    if is_breaker_setting(&body.key) {
        state.scheduler_mgr.reload_breaker().await;
    }

    // The canary period is applied in hot reload too: the running task adopts
    // the new period on the spot (issue #69).
    if body.key == crate::netcheck::SETTING_CANARY_INTERVAL_SECS {
        let interval = crate::netcheck::interval_from_setting(Some(body.value.as_str()));
        state.netcheck.set_interval(interval);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

// For GET queries via query params (easier from browser/fetch)
pub async fn get_setting_query(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<GetSettingQuery>,
) -> Result<Json<serde_json::Value>, String> {
    if let Some(key) = &query.key {
        let value = state.db.get_setting(key).await.map_err(|e| e.to_string())?;
        Ok(Json(serde_json::json!({ "key": key, "value": value })))
    } else {
        Ok(Json(serde_json::json!({ "error": "key required" })))
    }
}
