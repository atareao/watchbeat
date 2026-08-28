# Metric Consolidation Implementation Plan

> **For agentic workers:** Implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace real-time per-request bucket computation with a precomputed `consolidated_metrics` table populated by a background task, so chart queries for ranges >1h are O(1) reads instead of O(n) scans.

**Architecture:** A background task runs every hour, reads checks from the last hour, groups them into 80 buckets per period (6h, 12h, 24h, 7d, 15d, 30d, 3m, 6m, 1a), and upserts into `consolidated_metrics`. The 1h range still reads from `checks` directly (only 60–120 rows). All other ranges read from the precomputed table. The API response format (`TimelineBucket[]`) is identical, so the frontend needs zero changes.

**Tech Stack:** Rust + Axum + SQLite (sqlx 0.9, WAL mode), React 19 + TypeScript 7 + Vite 8 + Ant Design 6

## Global Constraints

- All SQLite migrations must be idempotent (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`)
- The `consolidated_metrics` table uses `INSERT OR REPLACE` (upsert via UNIQUE constraint)
- The 1h range (`hours <= 1`) must remain real-time from `checks` table
- Period strings are exactly: `'6h'`, `'12h'`, `'24h'`, `'7d'`, `'15d'`, `'30d'`, `'3m'`, `'6m'`, `'1a'`
- Each period always produces exactly 80 buckets
- Frontend API response shape `{ buckets: TimelineBucket[] }` must not change
- Default `retention_days` changes from 30 to 14

---

## File Structure

### Files to modify:
- `backend/src/models.rs` — Add `ConsolidatedMetricRow` struct
- `backend/src/db.rs` — Add table creation, `insert_consolidated_bucket`, `get_consolidated_buckets`, `delete_old_consolidated`; modify `cleanup_old_checks` default
- `backend/src/main.rs` — Add consolidation background task
- `backend/src/routes/checks.rs` — Modify `timeline()` handler to route to consolidated for ranges >1h

### Files unchanged:
- `frontend/src/pages/MonitorDetail.tsx` — No changes needed (API response format identical)
- `frontend/src/api/http.ts` — No changes needed
- `backend/src/config.rs` — No changes needed (retention_days is a DB setting, not a config field)

---

### Task 1: Add ConsolidatedMetric model + schema migration

**Files:**
- Modify: `backend/src/models.rs` — Add `ConsolidatedMetricRow` struct
- Modify: `backend/src/db.rs` — Add table + index creation in `Database::open()`

**Interfaces:**
- Produces: `ConsolidatedMetricRow` struct (sqlx::FromRow) with fields `monitor_id`, `period`, `bucket_start`, `up_pct`, `avg_response_time_ms`, `count`

- [ ] **Step 1: Add ConsolidatedMetricRow to models.rs**

Add this struct after the `TimelineBucket` block (around line 136):

```rust
// ───── Consolidated Metric (precomputed bucket) ─────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConsolidatedMetricRow {
    pub monitor_id: String,
    pub period: String,
    pub bucket_start: String,
    pub up_pct: f64,
    pub avg_response_time_ms: f64,
    pub count: i64,
}
```

- [ ] **Step 2: Add consolidated_metrics table creation in db.rs**

In `Database::open()`, after the `monitor_notifiers` table creation (around line 116), add:

```rust
let _ = sqlx::raw_sql(
    "CREATE TABLE IF NOT EXISTS consolidated_metrics (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        monitor_id TEXT NOT NULL,
        period TEXT NOT NULL,
        bucket_start TEXT NOT NULL,
        up_pct REAL NOT NULL DEFAULT 0.0,
        avg_response_time_ms REAL NOT NULL DEFAULT 0.0,
        count INTEGER NOT NULL DEFAULT 0,
        UNIQUE(monitor_id, period, bucket_start)
    )",
)
.execute(&pool)
.await;

let _ = sqlx::raw_sql(
    "CREATE INDEX IF NOT EXISTS idx_consolidated_lookup
     ON consolidated_metrics(monitor_id, period, bucket_start)",
)
.execute(&pool)
.await;
```

- [ ] **Step 3: Verify compilation**

```bash
cd backend && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add backend/src/models.rs backend/src/db.rs
git commit -m "feat: add consolidated_metrics table and ConsolidatedMetricRow model"
```

---

### Task 2: Add db.rs methods for consolidated_metrics CRUD

**Files:**
- Modify: `backend/src/db.rs` — Add `insert_consolidated_bucket`, `get_consolidated_buckets`, `delete_old_consolidated`

**Interfaces:**
- Consumes: `ConsolidatedMetricRow` from Task 1
- Produces: `insert_consolidated_bucket(monitor_id, period, bucket_start, up_pct, avg_rt, count)` — upsert
- Produces: `get_consolidated_buckets(monitor_id, period, since)` — returns `Vec<TimelineBucket>`
- Produces: `delete_old_consolidated(period, older_than)` — cleanup

- [ ] **Step 1: Add `insert_consolidated_bucket` method**

Add this method after the `cleanup_old_checks` method (around line 985):

```rust
pub async fn insert_consolidated_bucket(
    &self,
    monitor_id: &str,
    period: &str,
    bucket_start: &str,
    up_pct: f64,
    avg_response_time_ms: f64,
    count: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO consolidated_metrics (monitor_id, period, bucket_start, up_pct, avg_response_time_ms, count) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(monitor_id, period, bucket_start) DO UPDATE SET \
         up_pct=excluded.up_pct, avg_response_time_ms=excluded.avg_response_time_ms, count=excluded.count",
    )
    .bind(monitor_id)
    .bind(period)
    .bind(bucket_start)
    .bind(up_pct)
    .bind(avg_response_time_ms)
    .bind(count)
    .execute(&self.pool)
    .await
    .context("Failed to insert consolidated bucket")?;
    Ok(())
}
```

- [ ] **Step 2: Add `get_consolidated_buckets` method**

Add after `insert_consolidated_bucket`:

```rust
pub async fn get_consolidated_buckets(
    &self,
    monitor_id: &str,
    period: &str,
    since: &str,
) -> Result<Vec<TimelineBucket>> {
    let rows = sqlx::query_as::<_, ConsolidatedMetricRow>(
        "SELECT monitor_id, period, bucket_start, up_pct, avg_response_time_ms, count \
         FROM consolidated_metrics \
         WHERE monitor_id=? AND period=? AND bucket_start>=? \
         ORDER BY bucket_start ASC",
    )
    .bind(monitor_id)
    .bind(period)
    .bind(since)
    .fetch_all(&self.pool)
    .await
    .context("Failed to get consolidated buckets")?;

    let buckets: Vec<TimelineBucket> = rows
        .into_iter()
        .map(|r| {
            let dominant_status = if r.count == 0 {
                "no_data".to_string()
            } else if r.up_pct >= 50.0 {
                "up".to_string()
            } else {
                "down".to_string()
            };
            TimelineBucket {
                bucket_start: r.bucket_start,
                up_pct: (r.up_pct * 100.0).round() / 100.0,
                avg_response_time_ms: (r.avg_response_time_ms * 100.0).round() / 100.0,
                count: r.count,
                dominant_status,
            }
        })
        .collect();

    Ok(buckets)
}
```

- [ ] **Step 3: Add `delete_old_consolidated` method**

Add after `get_consolidated_buckets`:

```rust
pub async fn delete_old_consolidated(&self, period: &str, older_than: &str) -> Result<()> {
    sqlx::query(
        "DELETE FROM consolidated_metrics WHERE period=? AND bucket_start<?",
    )
    .bind(period)
    .bind(older_than)
    .execute(&self.pool)
    .await
    .context("Failed to delete old consolidated metrics")?;
    Ok(())
}
```

- [ ] **Step 4: Verify compilation**

```bash
cd backend && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add backend/src/db.rs
git commit -m "feat: add consolidated_metrics CRUD methods to Database"
```

---

### Task 3: Consolidation background task in main.rs

**Files:**
- Modify: `backend/src/main.rs` — Spawn a consolidation task alongside the scheduler

**Interfaces:**
- Consumes: `Database::insert_consolidated_bucket`, `Database::get_timeline` from Task 2
- Consumes: `ConsolidatedMetricRow` from Task 1

- [ ] **Step 1: Add the consolidation function and spawn it in main.rs**

After the scheduler spawn block (around line 128), add:

```rust
// ───── Metric Consolidation Task ─────
let db_for_consolidation = db.clone();
tokio::spawn(async move {
    consolidation_loop(db_for_consolidation).await;
});
```

Then add the consolidation functions after the `scheduler_loop` function (after line 209):

```rust
// ───── Consolidation Loop ─────

/// Period definitions: (period_label, duration_hours)
const CONSOLIDATION_PERIODS: &[(&str, i64)] = &[
    ("6h", 6),
    ("12h", 12),
    ("24h", 24),
    ("7d", 168),    // 7 * 24
    ("15d", 360),   // 15 * 24
    ("30d", 720),   // 30 * 24
    ("3m", 2160),   // 90 * 24
    ("6m", 4320),   // 180 * 24
    ("1a", 8640),   // 360 * 24
];

async fn consolidation_loop(db: Database) {
    // Small delay to let the server start
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    loop {
        let db = db.clone();
        let handle = tokio::spawn(async move {
            consolidation_iteration(&db).await;
        });

        match handle.await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("🔥 Consolidation task panicked: {}. Restarting...", e);
            }
        }

        // Run every hour
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

async fn consolidation_iteration(db: &Database) {
    let monitors = match db.list_monitors().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Consolidation: failed to load monitors: {}", e);
            return;
        }
    };

    let now = chrono::Utc::now();
    let one_hour_ago = (now - chrono::Duration::hours(1)).to_rfc3339();

    for monitor in &monitors {
        // Read checks from the last hour
        let points = match db.get_timeline(&monitor.id, &one_hour_ago).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "Consolidation: failed to get timeline for {}: {}",
                    monitor.name,
                    e
                );
                continue;
            }
        };

        if points.is_empty() {
            continue;
        }

        // For each period, divide the hour's checks into 80 buckets
        // and upsert into consolidated_metrics
        for &(period_label, period_hours) in CONSOLIDATION_PERIODS {
            let period_start = (now - chrono::Duration::hours(period_hours)).to_rfc3339();

            // We only have 1 hour of points. We need to distribute them across
            // the full period's 80 buckets. Each bucket covers period_hours/80 hours.
            let total_span_secs = period_hours * 3600;
            let bucket_size_secs = (total_span_secs as f64 / 80.0).ceil().max(1.0) as i64;

            let period_start_ts = match chrono::DateTime::parse_from_rfc3339(&period_start) {
                Ok(dt) => dt.with_timezone(&chrono::Utc).timestamp(),
                Err(_) => continue,
            };

            let mut buckets: Vec<TimelineBucket> = Vec::with_capacity(80);

            for i in 0..80 {
                let bucket_ts = period_start_ts + (i as i64 * bucket_size_secs);
                let bucket_end_ts = bucket_ts + bucket_size_secs;

                let bucket_start_str = match chrono::DateTime::from_timestamp(bucket_ts, 0) {
                    Some(dt) => dt.to_rfc3339(),
                    None => continue,
                };
                let bucket_end_str = match chrono::DateTime::from_timestamp(bucket_end_ts, 0) {
                    Some(dt) => dt.to_rfc3339(),
                    None => continue,
                };

                // Filter points that fall into this bucket
                let bucket_points: Vec<_> = points
                    .iter()
                    .filter(|p| p.checked_at >= bucket_start_str && p.checked_at < bucket_end_str)
                    .collect();

                let count = bucket_points.len() as i64;

                let (up_pct, avg_rt, dominant_status) = if count > 0 {
                    let up_count = bucket_points.iter().filter(|p| p.status == "up").count() as i64;
                    let up = (up_count as f64 / count as f64) * 100.0;
                    let avg = bucket_points
                        .iter()
                        .filter_map(|p| p.response_time_ms)
                        .map(|v| v as f64)
                        .sum::<f64>()
                        / count as f64;
                    let status = if up >= 50.0 { "up" } else { "down" };
                    (up, avg, status.to_string())
                } else {
                    (0.0, 0.0, "no_data".to_string())
                };

                buckets.push(TimelineBucket {
                    bucket_start: bucket_start_str,
                    up_pct: (up_pct * 100.0).round() / 100.0,
                    avg_response_time_ms: (avg_rt * 100.0).round() / 100.0,
                    count,
                    dominant_status,
                });
            }

            // Upsert all buckets for this period
            for bucket in &buckets {
                if let Err(e) = db
                    .insert_consolidated_bucket(
                        &monitor.id,
                        period_label,
                        &bucket.bucket_start,
                        bucket.up_pct,
                        bucket.avg_response_time_ms,
                        bucket.count,
                    )
                    .await
                {
                    tracing::warn!(
                        "Consolidation: failed to upsert bucket for {} period {}: {}",
                        monitor.name,
                        period_label,
                        e
                    );
                }
            }
        }
    }

    tracing::info!(
        "Consolidation: processed {} monitors across {} periods",
        monitors.len(),
        CONSOLIDATION_PERIODS.len()
    );
}
```

- [ ] **Step 2: Add TimelineBucket import if not already present**

Check that `TimelineBucket` is imported in `main.rs`. The file already imports from `watchbeat::models::CheckResult`. Add `TimelineBucket` to that import:

```rust
use watchbeat::models::{CheckResult, TimelineBucket};
```

- [ ] **Step 3: Verify compilation**

```bash
cd backend && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add backend/src/main.rs
git commit -m "feat: add hourly consolidation background task"
```

---

### Task 4: Update routes/checks.rs timeline handler

**Files:**
- Modify: `backend/src/routes/checks.rs` — Route ranges >1h to consolidated_metrics

**Interfaces:**
- Consumes: `Database::get_consolidated_buckets` from Task 2
- Consumes: `Database::get_timeline_buckets` (existing, for 1h range)

- [ ] **Step 1: Modify the `timeline()` handler**

Replace the entire `timeline` function (lines 54–84) with:

```rust
pub async fn timeline(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<serde_json::Value>, String> {
    // Determine the range: if hours is present and <= 1, use real-time checks
    let use_real_time = match query.hours {
        Some(h) => h <= 1,
        None => false,
    };

    if use_real_time {
        // 1h range: read from checks directly (only 60-120 rows)
        let since = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let bucket_seconds = query.bucket_seconds.unwrap_or(60).clamp(60, 86400 * 7);
        let buckets = state
            .db
            .get_timeline_buckets(&id, &since, bucket_seconds)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(Json(serde_json::json!({ "buckets": buckets })));
    }

    // For ranges >1h, determine period string and read from consolidated_metrics
    let (period, since) = if let Some(h) = query.hours {
        let period_str = match h {
            6 => "6h",
            12 => "12h",
            24 => "24h",
            _ => return Err(format!("Unsupported hours range: {}. Use 1, 6, 12, or 24.", h)),
        };
        (period_str, (chrono::Utc::now() - chrono::Duration::hours(h)).to_rfc3339())
    } else {
        let days = query.days.unwrap_or(1).clamp(1, 365);
        let period_str = match days {
            7 => "7d",
            15 => "15d",
            30 => "30d",
            90 => "3m",
            180 => "6m",
            365 => "1a",
            _ => return Err(format!("Unsupported days range: {}. Use 7, 15, 30, 90, 180, or 365.", days)),
        };
        (period_str, (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339())
    };

    let buckets = state
        .db
        .get_consolidated_buckets(&id, period, &since)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(serde_json::json!({ "buckets": buckets })))
}
```

- [ ] **Step 2: Update the unit tests**

The existing tests in `routes/checks.rs` test `TimelineQuery` parsing. They should still pass since we didn't change the query struct. However, the `test_timeline_query_days_default` test checks `days.clamp(1, 180)` but the original code clamps to `365`. Let's verify the test matches the current code.

Looking at the test (line 118): `let days = query.days.unwrap_or(1).clamp(1, 180);` — this test has a hardcoded clamp of 180, but the actual handler clamps to 365. This is a pre-existing inconsistency in the test (it tests the clamp logic independently, not the handler). The test should still pass as-is since it doesn't call the handler.

No test changes needed — the existing tests test `TimelineQuery` struct behavior, not the handler routing.

- [ ] **Step 3: Verify compilation**

```bash
cd backend && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 4: Run existing tests**

```bash
cd backend && cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/src/routes/checks.rs
git commit -m "feat: route chart ranges >1h to consolidated_metrics"
```

---

### Task 5: Adjust retention default from 30 to 14 days

**Files:**
- Modify: `backend/src/main.rs` — Change default `retention_days` from 30 to 14

- [ ] **Step 1: Change the retention default in scheduler_iteration**

In `main.rs`, find the `cleanup_old_checks` call (around line 280–290). Change the fallback default:

```rust
// Cleanup old checks (configurable retention, default 14 days)
let retention_days = db
    .get_setting("retention_days")
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(14);  // Changed from 30 to 14
if let Err(e) = db.cleanup_old_checks(retention_days).await {
    tracing::warn!("Scheduler: cleanup failed: {}", e);
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd backend && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add backend/src/main.rs
git commit -m "feat: reduce default checks retention from 30 to 14 days"
```

---

### Task 6: Verify frontend compatibility (no changes needed)

**Files:**
- None — verification only

The frontend `MonitorDetail.tsx` calls `fetchTimelineBuckets()` with the same parameters as before. The API response format `{ buckets: TimelineBucket[] }` is identical whether the data comes from `get_timeline_buckets()` (checks table) or `get_consolidated_buckets()` (consolidated_metrics table). The `TimelineBucket` struct has the same fields: `bucket_start`, `up_pct`, `avg_response_time_ms`, `count`, `dominant_status`.

The `MonitorStats` component fetches 24h, 30d, and 1y buckets — all of which now come from consolidated_metrics. No frontend changes needed.

- [ ] **Step 1: Verify frontend builds**

```bash
cd frontend && npm run build 2>&1
```
Expected: build succeeds with no errors.

- [ ] **Step 2: Verify full backend test suite**

```bash
cd backend && cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 3: Commit (if any changes were needed — none expected)**

```bash
git commit -m "chore: verify frontend compatibility with consolidated metrics API"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ Schema + migration (Task 1)
- ✅ db.rs methods for CRUD (Task 2)
- ✅ Consolidation background task (Task 3)
- ✅ Updated timeline handler routing (Task 4)
- ✅ 1h range stays real-time (Task 4, `use_real_time` check)
- ✅ Frontend unchanged (Task 6)
- ✅ Retention default changed to 14 (Task 5)

**2. Placeholder scan:**
- No TBD, TODO, or "implement later" found
- Every step has complete code
- All file paths are exact
- All function signatures are defined

**3. Type consistency:**
- `ConsolidatedMetricRow` used in Task 2 matches Task 1 definition
- `TimelineBucket` return type from `get_consolidated_buckets` matches existing `get_timeline_buckets` return type
- `insert_consolidated_bucket` signature matches usage in Task 3
- Period strings (`"6h"`, `"12h"`, etc.) consistent across Tasks 3 and 4