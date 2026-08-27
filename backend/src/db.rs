use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::models::{
    CheckResult, DashboardStatus, Monitor, MonitorRow, MonitorSummary, MonitorWithSummaryRow,
    Notifier, NotifierRow, StatusPage, StatusPageRow, TimelineBucket, TimelinePoint,
    TimelinePointRow,
};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    db_path: String,
}

impl Database {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create data directory")?;
        }

        let conn_opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(conn_opts)
            .await
            .context("Failed to open SQLite database")?;

        // Tables
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS monitors (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, monitor_type TEXT NOT NULL,
                target TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}',
                interval_seconds INTEGER NOT NULL DEFAULT 300, timeout_seconds INTEGER NOT NULL DEFAULT 30,
                enabled INTEGER NOT NULL DEFAULT 1, notifier_id TEXT,
                 confirmations_required INTEGER NOT NULL DEFAULT 0, failed_checks INTEGER NOT NULL DEFAULT 0,
                 tags TEXT NOT NULL DEFAULT '[]',
                 latency_threshold_ms INTEGER,
                 message_template_down TEXT,
                 message_template_latency TEXT,
                 message_template_up TEXT,
                 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
                status TEXT NOT NULL, status_code INTEGER,
                response_time_ms INTEGER NOT NULL DEFAULT 0, error_message TEXT, checked_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_checks_monitor ON checks(monitor_id, checked_at DESC);
            CREATE INDEX IF NOT EXISTS idx_checks_checked_at ON checks(checked_at);
            CREATE TABLE IF NOT EXISTS notifiers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, notifier_type TEXT NOT NULL,
                config_json TEXT NOT NULL DEFAULT '{}', enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS status_pages (
                id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, title TEXT NOT NULL,
                description TEXT, monitors TEXT NOT NULL DEFAULT '[]',
                public INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS monitor_notifiers (
                monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
                notifier_id TEXT NOT NULL REFERENCES notifiers(id) ON DELETE CASCADE,
                PRIMARY KEY (monitor_id, notifier_id)
            );",
        )
        .execute(&pool)
        .await
        .context("Failed to run migrations")?;

        // ALTER TABLE for existing DBs (ignore errors if columns already exist)
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN confirmations_required INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "ALTER TABLE monitors ADD COLUMN failed_checks INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN latency_threshold_ms INTEGER")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN message_template_down TEXT")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN message_template_latency TEXT")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN message_template_up TEXT")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN message_template_expiry TEXT")
            .execute(&pool)
            .await;

        // Heartbeat fields for monitors
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN token TEXT")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN grace_seconds INTEGER")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE monitors ADD COLUMN last_seen_at TEXT")
            .execute(&pool)
            .await;

        // Migrate heartbeats → monitors (idempotent: ignores if no heartbeats table)
        let _ = sqlx::raw_sql(
            "INSERT OR IGNORE INTO monitors (id, name, monitor_type, target, config_json, interval_seconds, timeout_seconds, enabled, notifier_id, token, grace_seconds, last_seen_at, created_at, updated_at) SELECT id, name, 'heartbeat', '', '{}', 300, 30, 1, notifier_id, token, grace_seconds, last_seen_at, created_at, updated_at FROM heartbeats"
        ).execute(&pool).await;

        // Drop heartbeats table (ignore if missing)
        let _ = sqlx::raw_sql("DROP TABLE IF EXISTS heartbeats")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("DROP INDEX IF EXISTS idx_heartbeats_name")
            .execute(&pool)
            .await;
        let _ = sqlx::raw_sql("DROP INDEX IF EXISTS idx_heartbeats_status")
            .execute(&pool)
            .await;

        // Unique indexes for name uniqueness (enforced at DB level)
        let _ =
            sqlx::raw_sql("CREATE UNIQUE INDEX IF NOT EXISTS idx_monitors_name ON monitors(name)")
                .execute(&pool)
                .await;
        let _ = sqlx::raw_sql(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_notifiers_name ON notifiers(name)",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_status_pages_title ON status_pages(title)",
        )
        .execute(&pool)
        .await;

        Ok(Self {
            pool,
            db_path: path.to_string_lossy().to_string(),
        })
    }

    // ───── Monitors ─────

    pub async fn list_monitors(&self) -> Result<Vec<Monitor>> {
        let rows = sqlx::query_as::<_, MonitorRow>(
            "SELECT id, name, monitor_type, target, config_json, interval_seconds, \
             timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, \
             latency_threshold_ms, message_template_down, message_template_latency, \
             message_template_up, message_template_expiry, created_at, updated_at FROM monitors ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list monitors")?;
        Ok(rows.into_iter().map(Monitor::from).collect())
    }

    pub async fn list_monitors_paginated(
        &self,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        filter_type: Option<&str>,
        filter_status: Option<&str>,
    ) -> Result<(Vec<Monitor>, i64, Vec<MonitorSummary>)> {
        let offset = (page - 1) * per_page;

        // Build dynamic WHERE clause
        let mut conditions: Vec<String> = Vec::new();
        let mut bindings: Vec<String> = Vec::new();

        if let Some(q) = search.filter(|s| !s.is_empty()) {
            conditions.push("(m.name LIKE ?1 OR m.target LIKE ?1)".to_string());
            bindings.push(format!("%{}%", q));
        }
        if let Some(t) = filter_type.filter(|s| !s.is_empty()) {
            let idx = bindings.len() + 1;
            conditions.push(format!("m.monitor_type = ?{}", idx));
            bindings.push(t.to_string());
        }
        if let Some(s) = filter_status.filter(|s| !s.is_empty()) {
            conditions.push("c.status = ?".to_string());
            bindings.push(s.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // For status filter we must use the subquery approach for the WHERE
        // Let's use a simpler approach with raw SQL string building
        let base_query = format!(
            "SELECT m.id, m.name, m.monitor_type, m.target, m.config_json, \
             m.interval_seconds, m.timeout_seconds, m.enabled, m.notifier_id, \
             m.confirmations_required, m.failed_checks, m.tags, \
             m.latency_threshold_ms, m.message_template_down, m.message_template_latency, \
             m.message_template_up, m.message_template_expiry, m.created_at, m.updated_at, \
             c.status AS last_status, c.response_time_ms AS last_response_time_ms, \
             c.checked_at AS last_checked_at \
             FROM monitors m \
             LEFT JOIN checks c ON c.id = (SELECT id FROM checks WHERE monitor_id = m.id ORDER BY checked_at DESC LIMIT 1) \
             {}",
            where_clause
        );

        let count_query = format!(
            "SELECT COUNT(*) FROM monitors m \
             LEFT JOIN checks c ON c.id = (SELECT id FROM checks WHERE monitor_id = m.id ORDER BY checked_at DESC LIMIT 1) \
             {}",
            where_clause
        );

        let order_query = format!("{} ORDER BY m.name LIMIT ? OFFSET ?", base_query);

        // Build and execute the count query
        let mut count_stmt = sqlx::query_as::<_, (i64,)>(sqlx::AssertSqlSafe(count_query.as_str()));
        for val in &bindings {
            count_stmt = count_stmt.bind(val);
        }
        let total: (i64,) = count_stmt
            .fetch_one(&self.pool)
            .await
            .context("Failed to count monitors")?;

        // Build and execute the data query
        let mut stmt =
            sqlx::query_as::<_, MonitorWithSummaryRow>(sqlx::AssertSqlSafe(order_query.as_str()));
        for val in &bindings {
            stmt = stmt.bind(val);
        }
        stmt = stmt.bind(per_page).bind(offset);
        let rows = stmt
            .fetch_all(&self.pool)
            .await
            .context("Failed to list monitors (paginated)")?;

        // Convert to Monitor and MonitorSummary, compute uptime for each
        let mut monitors = Vec::with_capacity(rows.len());
        let mut summaries = Vec::with_capacity(rows.len());

        for row in rows {
            let last_status = row.last_status.clone();
            let last_response_time_ms = row.last_response_time_ms;
            let last_checked_at = row.last_checked_at.clone();
            let m: Monitor = row.into();
            let summary = MonitorSummary {
                id: m.id.clone(),
                name: m.name.clone(),
                monitor_type: m.monitor_type.clone(),
                target: m.target.clone(),
                enabled: m.enabled,
                last_status,
                last_response_time_ms: last_response_time_ms.map(|v| v as u64),
                last_checked_at,
                uptime_7d: None,
                uptime_30d: None,
            };
            monitors.push(m);
            summaries.push(summary);
        }

        // Compute uptime for the returned monitors in batch
        let summary_ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
        let uptimes = self.batch_calculate_uptime(&summary_ids).await?;
        for s in &mut summaries {
            if let Some(u) = uptimes.get(s.id.as_str()) {
                s.uptime_7d = u.0;
                s.uptime_30d = u.1;
            }
        }

        Ok((monitors, total.0, summaries))
    }

    /// Calculate uptime for a batch of monitors in a single query per period
    async fn batch_calculate_uptime(
        &self,
        monitor_ids: &[&str],
    ) -> Result<std::collections::HashMap<String, (Option<f64>, Option<f64>)>> {
        use std::collections::HashMap;
        let mut result = HashMap::new();
        if monitor_ids.is_empty() {
            return Ok(result);
        }

        let cutoff_7d = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let cutoff_30d = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();

        for &id in monitor_ids {
            // 7d uptime
            let uptime_7d: Option<(i64, i64)> = sqlx::query_as(
                "SELECT CAST(COUNT(*) AS INTEGER) as total, \
                 CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS INTEGER) as up_count \
                 FROM checks WHERE monitor_id=? AND checked_at>=?",
            )
            .bind(id)
            .bind(&cutoff_7d)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to calculate 7d uptime")?;

            let u7 = match uptime_7d {
                Some((total, up)) if total > 0 => Some(up as f64 / total as f64 * 100.0),
                _ => None,
            };

            // 30d uptime
            let uptime_30d: Option<(i64, i64)> = sqlx::query_as(
                "SELECT CAST(COUNT(*) AS INTEGER) as total, \
                 CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS INTEGER) as up_count \
                 FROM checks WHERE monitor_id=? AND checked_at>=?",
            )
            .bind(id)
            .bind(&cutoff_30d)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to calculate 30d uptime")?;

            let u30 = match uptime_30d {
                Some((total, up)) if total > 0 => Some(up as f64 / total as f64 * 100.0),
                _ => None,
            };

            result.insert(id.to_string(), (u7, u30));
        }

        Ok(result)
    }

    pub async fn get_monitor(&self, id: &str) -> Result<Option<Monitor>> {
        let row = sqlx::query_as::<_, MonitorRow>(
            "SELECT id, name, monitor_type, target, config_json, interval_seconds, \
             timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, \
             latency_threshold_ms, message_template_down, message_template_latency, \
             message_template_up, message_template_expiry, created_at, updated_at FROM monitors WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get monitor")?;
        Ok(row.map(Monitor::from))
    }

    pub async fn check_name_unique(
        &self,
        table: &str,
        column: &str,
        value: &str,
        exclude_id: Option<&str>,
    ) -> Result<bool> {
        let count: i64 = match (table, column, exclude_id) {
            ("monitors", "name", None) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM monitors WHERE name=?")
                    .bind(value)
                    .fetch_one(&self.pool)
                    .await?
            }
            ("monitors", "name", Some(_)) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM monitors WHERE name=? AND id!=?")
                    .bind(value)
                    .bind(exclude_id.unwrap())
                    .fetch_one(&self.pool)
                    .await?
            }
            ("notifiers", "name", None) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM notifiers WHERE name=?")
                    .bind(value)
                    .fetch_one(&self.pool)
                    .await?
            }
            ("notifiers", "name", Some(_)) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM notifiers WHERE name=? AND id!=?")
                    .bind(value)
                    .bind(exclude_id.unwrap())
                    .fetch_one(&self.pool)
                    .await?
            }
            ("status_pages", "title", None) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM status_pages WHERE title=?")
                    .bind(value)
                    .fetch_one(&self.pool)
                    .await?
            }
            ("status_pages", "title", Some(_)) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM status_pages WHERE title=? AND id!=?")
                    .bind(value)
                    .bind(exclude_id.unwrap())
                    .fetch_one(&self.pool)
                    .await?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown table/column: {}/{}",
                    table,
                    column
                ))
            }
        };
        Ok(count == 0)
    }

    pub async fn create_monitor(&self, monitor: &Monitor) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&monitor.config_json)
            .context("Failed to serialize config_json")?;
        sqlx::query(
            "INSERT INTO monitors (id, name, monitor_type, target, config_json, interval_seconds, \
             timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, \
             latency_threshold_ms, message_template_down, message_template_latency, \
             message_template_up, message_template_expiry, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&monitor.id)
        .bind(&monitor.name)
        .bind(&monitor.monitor_type)
        .bind(&monitor.target)
        .bind(&config_json)
        .bind(monitor.interval_seconds)
        .bind(monitor.timeout_seconds)
        .bind(monitor.enabled as i32)
        .bind(&monitor.notifier_id)
        .bind(monitor.confirmations_required)
        .bind(monitor.failed_checks)
        .bind(serde_json::to_string(&monitor.tags).unwrap_or_else(|_| "[]".to_string()))
        .bind(monitor.latency_threshold_ms)
        .bind(&monitor.message_template_down)
        .bind(&monitor.message_template_latency)
        .bind(&monitor.message_template_up)
        .bind(&monitor.message_template_expiry)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .context("Failed to insert monitor")?;
        Ok(())
    }

    pub async fn update_monitor(&self, id: &str, monitor: &Monitor) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&monitor.config_json)
            .context("Failed to serialize config_json")?;
        let rows = sqlx::query(
            "UPDATE monitors SET name=?, monitor_type=?, target=?, config_json=?, \
             interval_seconds=?, timeout_seconds=?, enabled=?, notifier_id=?, \
             confirmations_required=?, failed_checks=?, tags=?, \
             latency_threshold_ms=?, message_template_down=?, message_template_latency=?, \
             message_template_up=?, message_template_expiry=?, updated_at=? WHERE id=?",
        )
        .bind(&monitor.name)
        .bind(&monitor.monitor_type)
        .bind(&monitor.target)
        .bind(&config_json)
        .bind(monitor.interval_seconds)
        .bind(monitor.timeout_seconds)
        .bind(monitor.enabled as i32)
        .bind(&monitor.notifier_id)
        .bind(monitor.confirmations_required)
        .bind(monitor.failed_checks)
        .bind(serde_json::to_string(&monitor.tags).unwrap_or_else(|_| "[]".to_string()))
        .bind(monitor.latency_threshold_ms)
        .bind(&monitor.message_template_down)
        .bind(&monitor.message_template_latency)
        .bind(&monitor.message_template_up)
        .bind(&monitor.message_template_expiry)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update monitor")?;
        Ok(rows.rows_affected() > 0)
    }

    pub async fn delete_monitor(&self, id: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM monitors WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete monitor")?;
        Ok(rows.rows_affected() > 0)
    }

    pub async fn toggle_monitor(&self, id: &str) -> Result<Option<bool>> {
        let current: Option<(i32,)> = sqlx::query_as("SELECT enabled FROM monitors WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to read monitor")?;
        let current_enabled = match current {
            Some((e,)) => e,
            None => return Ok(None),
        };
        let new_enabled = if current_enabled != 0 { 0 } else { 1 };
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE monitors SET enabled=?, updated_at=? WHERE id=?")
            .bind(new_enabled)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to toggle monitor")?;
        Ok(Some(new_enabled != 0))
    }

    pub async fn set_failed_checks(&self, id: &str, count: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE monitors SET failed_checks=?, updated_at=? WHERE id=?")
            .bind(count)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to set failed_checks")?;
        Ok(())
    }

    pub async fn reset_failed_checks(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE monitors SET failed_checks=0, updated_at=? WHERE id=?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to reset failed_checks")?;
        Ok(())
    }

    // ───── Checks ─────

    pub async fn insert_check(&self, check: &CheckResult) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO checks (monitor_id, status, status_code, response_time_ms, error_message, checked_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&check.monitor_id).bind(&check.status)
        .bind(check.status_code.map(|v| v as i64))
        .bind(check.response_time_ms).bind(&check.error_message).bind(&check.checked_at)
        .execute(&self.pool).await
        .context("Failed to insert check")?;
        Ok(result.last_insert_rowid() as i64)
    }

    pub async fn get_checks(
        &self,
        monitor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CheckResult>> {
        sqlx::query_as::<_, CheckResult>(
            "SELECT id, monitor_id, status, status_code, response_time_ms, error_message, checked_at \
             FROM checks WHERE monitor_id=? ORDER BY checked_at DESC LIMIT ? OFFSET ?",
        )
        .bind(monitor_id).bind(limit).bind(offset)
        .fetch_all(&self.pool).await
        .context("Failed to get checks")
    }

    pub async fn get_checks_count(&self, monitor_id: &str) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM checks WHERE monitor_id=?")
            .bind(monitor_id)
            .fetch_one(&self.pool)
            .await
            .context("Failed to count checks")?;
        Ok(count)
    }

    pub async fn get_latest_check(&self, monitor_id: &str) -> Result<Option<CheckResult>> {
        sqlx::query_as::<_, CheckResult>(
            "SELECT id, monitor_id, status, status_code, response_time_ms, error_message, checked_at \
             FROM checks WHERE monitor_id=? ORDER BY checked_at DESC LIMIT 1",
        )
        .bind(monitor_id).fetch_optional(&self.pool).await
        .context("Failed to get latest check")
    }

    pub async fn get_timeline(&self, monitor_id: &str, since: &str) -> Result<Vec<TimelinePoint>> {
        let rows = sqlx::query_as::<_, TimelinePointRow>(
            "SELECT checked_at, status, response_time_ms FROM checks \
             WHERE monitor_id=? AND checked_at>=? ORDER BY checked_at ASC",
        )
        .bind(monitor_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get timeline")?;
        Ok(rows.into_iter().map(TimelinePoint::from).collect())
    }

    pub async fn get_timeline_buckets(
        &self,
        monitor_id: &str,
        since: &str,
        _bucket_seconds: i64,
    ) -> Result<Vec<TimelineBucket>> {
        // Always divide the REQUESTED range into exactly 80 blocks
        let since_dt = match chrono::DateTime::parse_from_rfc3339(since) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => return Ok(Vec::new()),
        };
        let now = chrono::Utc::now();
        let total_span_secs = (now - since_dt).num_seconds().max(1);

        const TARGET_BLOCKS: usize = 80;
        let bucket_size_secs = (total_span_secs as f64 / TARGET_BLOCKS as f64)
            .ceil()
            .max(1.0) as i64;

        // Fetch all timeline points in range (already ordered ASC)
        let points = self.get_timeline(monitor_id, since).await?;

        // Build bucket boundaries as RFC 3339 strings for direct comparison
        // This avoids any chrono parsing issues with checked_at values
        let since_ts = since_dt.timestamp();
        let mut bucket_bounds: Vec<(i64, String)> = Vec::with_capacity(TARGET_BLOCKS + 1);
        for i in 0..=TARGET_BLOCKS {
            let ts = since_ts + (i as i64 * bucket_size_secs);
            let bound = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string());
            bucket_bounds.push((ts, bound));
        }

        let mut buckets: Vec<TimelineBucket> = Vec::with_capacity(TARGET_BLOCKS);

        for i in 0..TARGET_BLOCKS {
            let (_, start_str) = &bucket_bounds[i];
            let (_, end_str) = &bucket_bounds[i + 1];

            // Count points in this bucket using string comparison on checked_at
            // ISO 8601 strings sort lexicographically when same timezone
            let bucket_points: Vec<_> = points
                .iter()
                .filter(|p| p.checked_at >= *start_str && p.checked_at < *end_str)
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
                bucket_start: start_str.clone(),
                up_pct: (up_pct * 100.0).round() / 100.0,
                avg_response_time_ms: (avg_rt * 100.0).round() / 100.0,
                count,
                dominant_status,
            });
        }

        Ok(buckets)
    }

    pub async fn get_recent_checks_global(&self, limit: i64) -> Result<Vec<CheckResult>> {
        sqlx::query_as::<_, CheckResult>(
            "SELECT c.id, c.monitor_id, c.status, c.status_code, c.response_time_ms, c.error_message, c.checked_at \
             FROM checks c \
             INNER JOIN (SELECT monitor_id, MAX(checked_at) as max_checked FROM checks GROUP BY monitor_id) latest \
             ON c.monitor_id = latest.monitor_id AND c.checked_at = latest.max_checked \
             ORDER BY c.checked_at DESC LIMIT ?",
        )
        .bind(limit).fetch_all(&self.pool).await
        .context("Failed to get recent checks")
    }

    pub async fn get_monitor_summaries(&self) -> Result<Vec<MonitorSummary>> {
        let monitors = self.list_monitors().await?;
        let mut summaries = Vec::new();
        for m in monitors {
            let latest = self.get_latest_check(&m.id).await?;
            let uptime_7d = self.calculate_uptime(&m.id, 7).await?;
            let uptime_30d = self.calculate_uptime(&m.id, 30).await?;
            summaries.push(MonitorSummary {
                id: m.id,
                name: m.name,
                monitor_type: m.monitor_type,
                target: m.target,
                enabled: m.enabled,
                last_status: latest.as_ref().map(|c| c.status.clone()),
                last_response_time_ms: latest.as_ref().map(|c| c.response_time_ms as u64),
                last_checked_at: latest.map(|c| c.checked_at),
                uptime_7d,
                uptime_30d,
            });
        }
        Ok(summaries)
    }

    async fn calculate_uptime(&self, monitor_id: &str, days: i64) -> Result<Option<f64>> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let result: Option<(i64, i64)> = sqlx::query_as(
            "SELECT CAST(COUNT(*) AS INTEGER) as total, \
             CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS INTEGER) as up_count \
             FROM checks WHERE monitor_id=? AND checked_at>=?",
        )
        .bind(monitor_id)
        .bind(&cutoff)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to calculate uptime")?;
        match result {
            Some((total, up)) if total > 0 => Ok(Some(up as f64 / total as f64 * 100.0)),
            _ => Ok(None),
        }
    }

    pub async fn get_dashboard_status(&self) -> Result<DashboardStatus> {
        let monitors = self.list_monitors().await?;
        let total = monitors.len() as u64;
        let enabled = monitors.iter().filter(|m| m.enabled).count() as u64;
        let mut up = 0u64;
        let mut down = 0u64;
        let mut total_rt = 0u64;
        let mut rt_count = 0u64;
        for m in &monitors {
            if let Ok(Some(latest)) = self.get_latest_check(&m.id).await {
                match latest.status.as_str() {
                    "up" => up += 1,
                    _ => down += 1,
                }
                total_rt += latest.response_time_ms as u64;
                rt_count += 1;
            }
        }
        let cutoff_24h = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let checks_24h: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checks WHERE checked_at>=?")
            .bind(&cutoff_24h)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        Ok(DashboardStatus {
            total_monitors: total,
            enabled_monitors: enabled,
            up_monitors: up,
            down_monitors: down,
            total_checks_24h: checks_24h as u64,
            avg_response_time_24h: if rt_count > 0 {
                Some(total_rt.checked_div(rt_count).unwrap_or(0))
            } else {
                None
            },
        })
    }

    pub async fn cleanup_old_checks(&self, retention_days: i64) -> Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();
        sqlx::query("DELETE FROM checks WHERE checked_at<?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await
            .context("Failed to cleanup old checks")?;
        Ok(())
    }

    // ───── Notifiers ─────

    pub async fn list_notifiers(&self) -> Result<Vec<Notifier>> {
        let rows = sqlx::query_as::<_, NotifierRow>(
            "SELECT id, name, notifier_type, config_json, enabled, created_at, updated_at FROM notifiers ORDER BY name",
        ).fetch_all(&self.pool).await.context("Failed to list notifiers")?;
        Ok(rows.into_iter().map(Notifier::from).collect())
    }

    pub async fn get_notifier(&self, id: &str) -> Result<Option<Notifier>> {
        let row = sqlx::query_as::<_, NotifierRow>(
            "SELECT id, name, notifier_type, config_json, enabled, created_at, updated_at FROM notifiers WHERE id=?",
        ).bind(id).fetch_optional(&self.pool).await.context("Failed to get notifier")?;
        Ok(row.map(Notifier::from))
    }

    pub async fn upsert_notifier(&self, id: &str, notifier: &Notifier) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let config_json =
            serde_json::to_string(&notifier.config_json).context("Failed to serialize config")?;
        sqlx::query(
            "INSERT INTO notifiers (id, name, notifier_type, config_json, enabled, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, notifier_type=excluded.notifier_type, \
             config_json=excluded.config_json, enabled=excluded.enabled, updated_at=excluded.updated_at",
        ).bind(id).bind(&notifier.name).bind(&notifier.notifier_type).bind(&config_json)
        .bind(notifier.enabled as i32).bind(&now).bind(&now)
        .execute(&self.pool).await.context("Failed to upsert notifier")?;
        Ok(())
    }

    pub async fn delete_notifier(&self, id: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM notifiers WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete notifier")?;
        Ok(rows.rows_affected() > 0)
    }

    // ───── Settings ─────

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        sqlx::query_scalar("SELECT value FROM settings WHERE key=?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get setting")
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value")
            .bind(key).bind(value).execute(&self.pool).await.context("Failed to set setting")?;
        Ok(())
    }

    // ───── Monitor-Notifier bindings ─────

    pub async fn set_monitor_notifiers(
        &self,
        monitor_id: &str,
        notifier_ids: &[String],
    ) -> Result<()> {
        sqlx::query("DELETE FROM monitor_notifiers WHERE monitor_id=?")
            .bind(monitor_id)
            .execute(&self.pool)
            .await
            .context("Failed to clear monitor notifiers")?;
        for nid in notifier_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO monitor_notifiers (monitor_id, notifier_id) VALUES (?, ?)",
            )
            .bind(monitor_id)
            .bind(nid)
            .execute(&self.pool)
            .await
            .context("Failed to link monitor notifier")?;
        }
        Ok(())
    }

    pub async fn get_monitor_notifier_ids(&self, monitor_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT notifier_id FROM monitor_notifiers WHERE monitor_id=?")
                .bind(monitor_id)
                .fetch_all(&self.pool)
                .await
                .context("Failed to get monitor notifiers")?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    // ───── Status Pages ─────

    pub async fn list_status_pages(&self) -> Result<Vec<StatusPage>> {
        let rows = sqlx::query_as::<_, StatusPageRow>(
            "SELECT id, slug, title, description, monitors, public, created_at, updated_at FROM status_pages ORDER BY title",
        ).fetch_all(&self.pool).await.context("Failed to list status pages")?;
        Ok(rows.into_iter().map(StatusPage::from).collect())
    }

    pub async fn get_status_page(&self, id: &str) -> Result<Option<StatusPage>> {
        let row = sqlx::query_as::<_, StatusPageRow>(
            "SELECT id, slug, title, description, monitors, public, created_at, updated_at FROM status_pages WHERE id=?",
        ).bind(id).fetch_optional(&self.pool).await.context("Failed to get status page")?;
        Ok(row.map(StatusPage::from))
    }

    pub async fn get_status_page_by_slug(&self, slug: &str) -> Result<Option<StatusPage>> {
        let row = sqlx::query_as::<_, StatusPageRow>(
            "SELECT id, slug, title, description, monitors, public, created_at, updated_at FROM status_pages WHERE slug=?",
        ).bind(slug).fetch_optional(&self.pool).await.context("Failed to get status page by slug")?;
        Ok(row.map(StatusPage::from))
    }

    pub async fn upsert_status_page(&self, id: &str, page: &StatusPage) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let monitors = serde_json::to_string(&page.monitors).unwrap_or_else(|_| "[]".into());
        sqlx::query(
            "INSERT INTO status_pages (id, slug, title, description, monitors, public, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET slug=excluded.slug, title=excluded.title, \
             description=excluded.description, monitors=excluded.monitors, public=excluded.public, updated_at=excluded.updated_at",
        ).bind(id).bind(&page.slug).bind(&page.title).bind(&page.description).bind(&monitors)
        .bind(page.public as i32).bind(&now).bind(&now)
        .execute(&self.pool).await.context("Failed to upsert status page")?;
        Ok(())
    }

    pub async fn delete_status_page(&self, id: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM status_pages WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete status page")?;
        Ok(rows.rows_affected() > 0)
    }

    // ───── Heartbeat pulse (public endpoint) ─────

    /// Record a heartbeat pulse: find monitor by token, insert a check, update last_seen_at.
    pub async fn record_heartbeat_pulse(&self, token: &str) -> Result<Option<Monitor>> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = sqlx::query_as::<_, MonitorRow>(
            "SELECT id, name, monitor_type, target, config_json, interval_seconds, timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, latency_threshold_ms, message_template_down, message_template_latency, message_template_up, message_template_expiry, token, grace_seconds, last_seen_at, created_at, updated_at FROM monitors WHERE token=? AND monitor_type='heartbeat'",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to find monitor by token")?;

        let monitor = match row {
            Some(r) => Monitor::from(r),
            None => return Ok(None),
        };

        // Insert a check record for the pulse
        let check = CheckResult {
            id: 0,
            monitor_id: monitor.id.clone(),
            status: "ok".into(),
            status_code: None,
            response_time_ms: 0,
            error_message: None,
            checked_at: now.clone(),
        };
        sqlx::query(
            "INSERT INTO checks (monitor_id, status, status_code, response_time_ms, error_message, checked_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&check.monitor_id)
        .bind(&check.status)
        .bind(check.status_code.map(|v| v as i64))
        .bind(check.response_time_ms)
        .bind(&check.error_message)
        .bind(&check.checked_at)
        .execute(&self.pool)
        .await
        .context("Failed to insert heartbeat check")?;

        // Update last_seen_at
        sqlx::query("UPDATE monitors SET last_seen_at=?, updated_at=? WHERE id=?")
            .bind(&now)
            .bind(&now)
            .bind(&monitor.id)
            .execute(&self.pool)
            .await
            .context("Failed to update last_seen_at")?;

        Ok(Some(Monitor {
            last_seen_at: Some(now),
            ..monitor
        }))
    }

    // ───── Backup ─────

    pub async fn backup(&self, output_path: &std::path::Path) -> Result<()> {
        let db_path = &self.db_path;

        // Force WAL checkpoint
        sqlx::raw_sql("PRAGMA wal_checkpoint(TRUNCATE);")
            .execute(&self.pool)
            .await
            .ok();

        // Copy the database file (and WAL/SHM if present)
        tokio::fs::copy(db_path, output_path)
            .await
            .context("Failed to copy database file")?;

        let wal_path = format!("{}-wal", db_path);
        if tokio::fs::try_exists(&wal_path).await.unwrap_or(false) {
            let wal_out = format!("{}-wal", output_path.display());
            let _ = tokio::fs::copy(&wal_path, &wal_out).await;
        }

        Ok(())
    }
}
