use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AppState;

#[derive(Deserialize)]
pub struct ChecksQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ChecksQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    let checks = state
        .db
        .get_checks(&id, limit, offset)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({ "checks": checks })))
}

#[derive(Deserialize)]
pub struct TimelineQuery {
    pub days: Option<i64>,
}

pub async fn timeline(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let days = query.days.unwrap_or(1).max(1).min(90);
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();

    let timeline = state
        .db
        .get_timeline(&id, &since)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({ "timeline": timeline })))
}

#[derive(Deserialize)]
pub struct RecentQuery {
    pub limit: Option<i64>,
}

pub async fn recent_global(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecentQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let limit = query.limit.unwrap_or(50).min(200);

    let checks = state
        .db
        .get_recent_checks_global(limit)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({ "checks": checks })))
}