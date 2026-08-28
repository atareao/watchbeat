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
        let h = h.clamp(1, 24 * 365); // max ~1 year in hours
        (chrono::Utc::now() - chrono::Duration::hours(h)).to_rfc3339()
    } else {
        let days = query.days.unwrap_or(1).clamp(1, 365);
        (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    };

    // Determine if we should use consolidated metrics or real-time checks
    // 1h → real-time from checks (few rows, fast)
    // 6h+ → consolidated_metrics (precomputed)
    let use_consolidated = match (query.hours, query.days) {
        (Some(h), _) if h <= 1 => false, // 1h → real-time
        (Some(_), _) => true,            // 6h, 12h, 24h → consolidated
        (_, Some(_)) => true,            // any days → consolidated
        (None, None) => false,           // default → real-time
    };

    if use_consolidated {
        // Map the range to a period string
        let period = match (query.hours, query.days) {
            (Some(6), _) => "6h",
            (Some(12), _) => "12h",
            (Some(24), _) => "24h",
            (_, Some(7)) => "7d",
            (_, Some(15)) => "15d",
            (_, Some(30)) => "30d",
            (_, Some(90)) => "3m",
            (_, Some(180)) => "6m",
            (_, Some(365)) => "1a",
            // Fallback: derive from bucket_seconds
            _ => {
                let bs = query.bucket_seconds.unwrap_or(900);
                match bs {
                    0..=300 => "6h",
                    301..=600 => "12h",
                    601..=900 => "24h",
                    901..=7200 => "7d",
                    7201..=14400 => "15d",
                    14401..=28800 => "30d",
                    28801..=86400 => "3m",
                    86401..=172800 => "6m",
                    _ => "1a",
                }
            }
        };

        let rows = state
            .db
            .get_consolidated_buckets(&id, period, &since)
            .await
            .map_err(|e| e.to_string())?;

        // Take only the last 60 buckets (consolidation creates 60 per period per run)
        let rows: Vec<_> = rows.into_iter().rev().take(60).rev().collect();

        // Add dominant_status derived from up_pct (same logic as TimelineBucket)
        let buckets: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                let dominant_status = if r.count == 0 {
                    "no_data"
                } else if r.up_pct >= 50.0 {
                    "up"
                } else {
                    "down"
                };
                serde_json::json!({
                    "bucket_start": r.bucket_start,
                    "up_pct": r.up_pct,
                    "avg_response_time_ms": r.avg_response_time_ms,
                    "count": r.count,
                    "dominant_status": dominant_status,
                })
            })
            .collect();

        return Ok(Json(serde_json::json!({ "buckets": buckets })));
    }

    // 1h or default: real-time from checks (existing behavior)
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
        let days = query.days.unwrap_or(1).clamp(1, 365);
        assert_eq!(days, 365);
    }

    #[test]
    fn test_timeline_query_hours() {
        let query = TimelineQuery {
            days: None,
            hours: Some(6),
            bucket_seconds: None,
        };
        let h = query.hours.unwrap().clamp(1, 24 * 180);
        assert_eq!(h, 6);
    }

    #[test]
    fn test_timeline_query_hours_clamp() {
        let query = TimelineQuery {
            days: None,
            hours: Some(9000),
            bucket_seconds: None,
        };
        let h = query.hours.unwrap().clamp(1, 24 * 365);
        assert_eq!(h, 24 * 365);
    }
}
