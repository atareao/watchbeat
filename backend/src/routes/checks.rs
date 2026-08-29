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
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let checks = state
        .db
        .get_checks(&id, limit, offset)
        .await
        .map_err(|e| e.to_string())?;

    let total = state
        .db
        .get_checks_count(&id)
        .await
        .map_err(|e| e.to_string())?;

    let total_pages = (total as f64 / limit as f64).ceil() as i64;
    let page = (offset / limit) + 1;

    Ok(Json(serde_json::json!({
        "checks": checks,
        "total": total,
        "page": page,
        "per_page": limit,
        "total_pages": total_pages,
    })))
}

#[derive(Deserialize)]
pub struct TimelineQuery {
    pub days: Option<i64>,
    pub hours: Option<i64>,
    pub bucket_seconds: Option<i64>,
}

pub async fn timeline(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let since = if let Some(h) = query.hours {
        let h = h.clamp(1, 24 * 30); // max ~30 days in hours
        (chrono::Utc::now() - chrono::Duration::hours(h)).to_rfc3339()
    } else {
        let days = query.days.unwrap_or(1).clamp(1, 30);
        (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    };

    // All ranges use real-time from checks (no consolidated_metrics)
    if let Some(bucket_seconds) = query.bucket_seconds {
        let bucket_seconds = bucket_seconds.clamp(60, 86400 * 7); // 1 min to 7 days
        let buckets = state
            .db
            .get_timeline_buckets(&id, &since, bucket_seconds)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(Json(serde_json::json!({ "buckets": buckets })));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_timeline_query_days_default() {
        let query = TimelineQuery {
            days: None,
            hours: None,
            bucket_seconds: None,
        };
        let days = query.days.unwrap_or(1).clamp(1, 180);
        assert_eq!(days, 1);
        let since = (Utc::now() - Duration::days(days)).to_rfc3339();
        let parsed = chrono::DateTime::parse_from_rfc3339(&since).unwrap();
        let diff = Utc::now().signed_duration_since(parsed);
        assert!(diff.num_hours() >= 23 && diff.num_hours() <= 25);
    }

    #[test]
    fn test_timeline_query_days_clamp() {
        let query = TimelineQuery {
            days: Some(400),
            hours: None,
            bucket_seconds: None,
        };
        let days = query.days.unwrap_or(1).clamp(1, 30);
        assert_eq!(days, 30);
    }

    #[test]
    fn test_timeline_query_hours() {
        let query = TimelineQuery {
            days: None,
            hours: Some(6),
            bucket_seconds: None,
        };
        let h = query.hours.unwrap().clamp(1, 24 * 30);
        assert_eq!(h, 6);
    }

    #[test]
    fn test_timeline_query_hours_clamp() {
        let query = TimelineQuery {
            days: None,
            hours: Some(9000),
            bucket_seconds: None,
        };
        let h = query.hours.unwrap().clamp(1, 24 * 30);
        assert_eq!(h, 24 * 30);
    }
}
