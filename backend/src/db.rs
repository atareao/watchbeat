use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::models::{
    CheckResult, DashboardStatus, Heartbeat, HeartbeatRow, Monitor, MonitorRow, MonitorSummary,
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
            CREATE TABLE IF NOT EXISTS heartbeats (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, token TEXT NOT NULL UNIQUE,
                grace_seconds INTEGER NOT NULL DEFAULT 3600,
                last_seen_at TEXT, status TEXT NOT NULL DEFAULT 'pending',
                notifier_id TEXT,
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
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_heartbeats_name ON heartbeats(name)",
        )
        .execute(&pool)
        .await;
        let _ = sqlx::raw_sql(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_status_pages_title ON status_pages(title)",
        )
        .execute(&pool)
        .await;

        // Index for ORDER BY on heartbeats and status_pages
        let _ =
            sqlx::raw_sql("CREATE INDEX IF NOT EXISTS idx_heartbeats_status ON heartbeats(status)")
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
             created_at, updated_at FROM monitors ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list monitors")?;
        Ok(rows.into_iter().map(Monitor::from).collect())
    }

    pub async fn get_monitor(&self, id: &str) -> Result<Option<Monitor>> {
        let row = sqlx::query_as::<_, MonitorRow>(
            "SELECT id, name, monitor_type, target, config_json, interval_seconds, \
             timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, \
             created_at, updated_at FROM monitors WHERE id = ?",
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
            ("heartbeats", "name", None) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM heartbeats WHERE name=?")
                    .bind(value)
                    .fetch_one(&self.pool)
                    .await?
            }
            ("heartbeats", "name", Some(_)) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM heartbeats WHERE name=? AND id!=?")
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
             timeout_seconds, enabled, notifier_id, confirmations_required, failed_checks, tags, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&monitor.id).bind(&monitor.name).bind(&monitor.monitor_type)
        .bind(&monitor.target).bind(&config_json)
        .bind(monitor.interval_seconds).bind(monitor.timeout_seconds)
        .bind(monitor.enabled as i32).bind(&monitor.notifier_id)
        .bind(monitor.confirmations_required).bind(monitor.failed_checks)
        .bind(serde_json::to_string(&monitor.tags).unwrap_or_else(|_| "[]".to_string()))
        .bind(&now).bind(&now)
        .execute(&self.pool).await
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
             confirmations_required=?, failed_checks=?, tags=?, updated_at=? WHERE id=?",
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
        bucket_seconds: i64,
    ) -> Result<Vec<TimelineBucket>> {
        // First, find the actual data range for this monitor within the time window
        let actual_span: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT MIN(checked_at), MAX(checked_at) FROM checks \
             WHERE monitor_id=? AND checked_at>=?",
        )
        .bind(monitor_id)
        .bind(since)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get actual data range")?;

        // If no data, return empty
        let (min_checked, max_checked) = match actual_span {
            Some((Some(min), Some(max))) => (min, max),
            _ => return Ok(Vec::new()),
        };

        // Calculate actual span in seconds using julianday
        let span_seconds: f64 = sqlx::query_scalar(
            "SELECT (julianday(?) - julianday(?)) * 86400",
        )
        .bind(&max_checked)
        .bind(&min_checked)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0.0);

        // Target ~80 blocks based on actual data span
        // If data span is smaller than the requested range, we use smaller buckets
        // If data span is larger, we use larger buckets
        // Clamped between 60s (min) and the requested bucket_seconds (max)
        let target_blocks: i64 = 80;
        let ideal_bucket = (span_seconds as i64).max(1) / target_blocks;
        let effective_bucket = ideal_bucket.clamp(60, bucket_seconds);

        // Bucket by integer division of unix timestamp
        // Using julianday instead of strftime because strftime doesn't handle
        // timezone offsets (+00:00) in RFC 3339 dates reliably.
        // Formula: (julianday - 2440587.5) * 86400 = unix timestamp
        let rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
            "SELECT \
             (CAST((julianday(checked_at) - 2440587.5) * 86400 AS INTEGER) / ?) * ? AS bucket_unix, \
             COUNT(*) AS total, \
             SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS up_count, \
             AVG(response_time_ms) AS avg_rt \
             FROM checks \
             WHERE monitor_id=? AND checked_at>=? \
             GROUP BY bucket_unix \
             ORDER BY bucket_unix ASC",
        )
        .bind(effective_bucket)
        .bind(effective_bucket)
        .bind(monitor_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get timeline buckets")?;

        Ok(rows
            .into_iter()
            .map(|(bucket_unix, total, up_count, avg_rt)| {
                let up_pct = if total > 0 {
                    (up_count as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let dominant_status = if up_pct >= 50.0 {
                    "up"
                } else if up_count < total {
                    "down"
                } else {
                    "error"
                };
                // Convert unix timestamp to RFC 3339
                let bucket_start = chrono::DateTime::from_timestamp(bucket_unix, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| "unknown".to_string());
                TimelineBucket {
                    bucket_start,
                    up_pct: (up_pct * 100.0).round() / 100.0, // 2 decimal places
                    avg_response_time_ms: (avg_rt * 100.0).round() / 100.0,
                    count: total,
                    dominant_status: dominant_status.to_string(),
                }
            })
            .collect())
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

    // ───── Heartbeats ─────

    pub async fn list_heartbeats(&self) -> Result<Vec<Heartbeat>> {
        let rows = sqlx::query_as::<_, HeartbeatRow>(
            "SELECT id, name, token, grace_seconds, last_seen_at, status, notifier_id, created_at, updated_at FROM heartbeats ORDER BY name",
        ).fetch_all(&self.pool).await.context("Failed to list heartbeats")?;
        Ok(rows.into_iter().map(Heartbeat::from).collect())
    }

    pub async fn get_heartbeat(&self, id: &str) -> Result<Option<Heartbeat>> {
        let row = sqlx::query_as::<_, HeartbeatRow>(
            "SELECT id, name, token, grace_seconds, last_seen_at, status, notifier_id, created_at, updated_at FROM heartbeats WHERE id=?",
        ).bind(id).fetch_optional(&self.pool).await.context("Failed to get heartbeat")?;
        Ok(row.map(Heartbeat::from))
    }

    pub async fn get_heartbeat_by_token(&self, token: &str) -> Result<Option<Heartbeat>> {
        let row = sqlx::query_as::<_, HeartbeatRow>(
            "SELECT id, name, token, grace_seconds, last_seen_at, status, notifier_id, created_at, updated_at FROM heartbeats WHERE token=?",
        ).bind(token).fetch_optional(&self.pool).await.context("Failed to get heartbeat by token")?;
        Ok(row.map(Heartbeat::from))
    }

    pub async fn upsert_heartbeat(&self, id: &str, hb: &Heartbeat) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO heartbeats (id, name, token, grace_seconds, last_seen_at, status, notifier_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, token=excluded.token, \
             grace_seconds=excluded.grace_seconds, last_seen_at=excluded.last_seen_at, \
             status=excluded.status, notifier_id=excluded.notifier_id, updated_at=excluded.updated_at",
        ).bind(id).bind(&hb.name).bind(&hb.token).bind(hb.grace_seconds).bind(&hb.last_seen_at)
        .bind(&hb.status).bind(&hb.notifier_id).bind(&now).bind(&now)
        .execute(&self.pool).await.context("Failed to upsert heartbeat")?;
        Ok(())
    }

    pub async fn delete_heartbeat(&self, id: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM heartbeats WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete heartbeat")?;
        Ok(rows.rows_affected() > 0)
    }

    pub async fn touch_heartbeat(&self, token: &str) -> Result<Option<Heartbeat>> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query_as::<_, HeartbeatRow>(
            "SELECT id, name, token, grace_seconds, last_seen_at, status, notifier_id, created_at, updated_at FROM heartbeats WHERE token=?",
        ).bind(token).fetch_optional(&self.pool).await.context("Failed to find heartbeat")?;
        if let Some(r) = row {
            sqlx::query(
                "UPDATE heartbeats SET last_seen_at=?, status='ok', updated_at=? WHERE token=?",
            )
            .bind(&now)
            .bind(&now)
            .bind(token)
            .execute(&self.pool)
            .await
            .context("Failed to touch heartbeat")?;
            let hb = Heartbeat::from(r);
            Ok(Some(Heartbeat {
                last_seen_at: Some(now),
                status: "ok".into(),
                ..hb
            }))
        } else {
            Ok(None)
        }
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
