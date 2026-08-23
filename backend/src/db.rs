use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::Mutex;

use crate::models::{CheckResult, Monitor, Notifier};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create data directory")?;
        }

        let conn = Connection::open(path).context("Failed to open SQLite database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMAs")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations().await?;
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS monitors (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                monitor_type TEXT NOT NULL,
                target TEXT NOT NULL,
                config_json TEXT NOT NULL DEFAULT '{}',
                interval_seconds INTEGER NOT NULL DEFAULT 300,
                timeout_seconds INTEGER NOT NULL DEFAULT 30,
                enabled INTEGER NOT NULL DEFAULT 1,
                notifier_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                status_code INTEGER,
                response_time_ms INTEGER NOT NULL DEFAULT 0,
                error_message TEXT,
                checked_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_checks_monitor
                ON checks(monitor_id, checked_at DESC);

            CREATE INDEX IF NOT EXISTS idx_checks_checked_at
                ON checks(checked_at);

            CREATE TABLE IF NOT EXISTS notifiers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                notifier_type TEXT NOT NULL,
                config_json TEXT NOT NULL DEFAULT '{}',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .context("Failed to run database migrations")?;

        Ok(())
    }

    // ───── Monitors ─────

    pub async fn list_monitors(&self) -> Result<Vec<Monitor>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, monitor_type, target, config_json, interval_seconds, \
                 timeout_seconds, enabled, notifier_id, created_at, updated_at FROM monitors ORDER BY name",
            )
            .context("Failed to prepare list_monitors")?;

        let monitors = stmt
            .query_map([], |row| {
                let config_json: String = row.get(4)?;
                Ok(Monitor {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    monitor_type: row.get(2)?,
                    target: row.get(3)?,
                    config_json: serde_json::from_str(&config_json)
                        .unwrap_or(serde_json::Value::Object(Default::default())),
                    interval_seconds: row.get(5)?,
                    timeout_seconds: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? != 0,
                    notifier_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .context("Failed to query monitors")?;

        let mut result = Vec::new();
        for m in monitors {
            result.push(m?);
        }
        Ok(result)
    }

    pub async fn get_monitor(&self, id: &str) -> Result<Option<Monitor>> {
        let conn = self.conn.lock().await;
        let monitor = conn
            .query_row(
                "SELECT id, name, monitor_type, target, config_json, interval_seconds, \
                 timeout_seconds, enabled, notifier_id, created_at, updated_at FROM monitors WHERE id = ?1",
                params![id],
                |row| {
                    let config_json: String = row.get(4)?;
                    Ok(Monitor {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        monitor_type: row.get(2)?,
                        target: row.get(3)?,
                        config_json: serde_json::from_str(&config_json)
                            .unwrap_or(serde_json::Value::Object(Default::default())),
                        interval_seconds: row.get(5)?,
                        timeout_seconds: row.get(6)?,
                        enabled: row.get::<_, i32>(7)? != 0,
                        notifier_id: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()
            .context("Failed to query monitor")?;
        Ok(monitor)
    }

    pub async fn create_monitor(&self, monitor: &Monitor) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&monitor.config_json)
            .context("Failed to serialize config_json")?;

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO monitors (id, name, monitor_type, target, config_json, interval_seconds, \
             timeout_seconds, enabled, notifier_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                monitor.id,
                monitor.name,
                monitor.monitor_type,
                monitor.target,
                config_json,
                monitor.interval_seconds,
                monitor.timeout_seconds,
                monitor.enabled as i32,
                monitor.notifier_id,
                now,
                now,
            ],
        )
        .context("Failed to insert monitor")?;
        Ok(())
    }

    pub async fn update_monitor(&self, id: &str, monitor: &Monitor) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&monitor.config_json)
            .context("Failed to serialize config_json")?;

        let conn = self.conn.lock().await;
        let rows = conn
            .execute(
                "UPDATE monitors SET name=?1, monitor_type=?2, target=?3, config_json=?4, \
                 interval_seconds=?5, timeout_seconds=?6, enabled=?7, notifier_id=?8, updated_at=?9 \
                 WHERE id=?10",
                params![
                    monitor.name,
                    monitor.monitor_type,
                    monitor.target,
                    config_json,
                    monitor.interval_seconds,
                    monitor.timeout_seconds,
                    monitor.enabled as i32,
                    monitor.notifier_id,
                    now,
                    id,
                ],
            )
            .context("Failed to update monitor")?;
        Ok(rows > 0)
    }

    pub async fn delete_monitor(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn
            .execute("DELETE FROM monitors WHERE id = ?1", params![id])
            .context("Failed to delete monitor")?;
        Ok(rows > 0)
    }

    pub async fn toggle_monitor(&self, id: &str) -> Result<Option<bool>> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let rows = conn
            .execute(
                "UPDATE monitors SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END, updated_at=?1 WHERE id=?2",
                params![now, id],
            )
            .context("Failed to toggle monitor")?;
        if rows == 0 {
            return Ok(None);
        }
        let enabled: bool = conn
            .query_row(
                "SELECT enabled FROM monitors WHERE id=?1",
                params![id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .optional()
            .context("Failed to read monitor after toggle")?
            .unwrap_or(false);
        Ok(Some(enabled))
    }

    // ───── Checks ─────

    pub async fn insert_check(&self, check: &CheckResult) -> Result<i64> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO checks (monitor_id, status, status_code, response_time_ms, error_message, checked_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                check.monitor_id,
                check.status,
                check.status_code,
                check.response_time_ms as i64,
                check.error_message,
                check.checked_at,
            ],
        )
        .context("Failed to insert check")?;
        Ok(conn.last_insert_rowid())
    }

    pub async fn get_checks(
        &self,
        monitor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CheckResult>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, monitor_id, status, status_code, response_time_ms, error_message, checked_at \
                 FROM checks WHERE monitor_id=?1 ORDER BY checked_at DESC LIMIT ?2 OFFSET ?3",
            )
            .context("Failed to prepare get_checks")?;

        let checks = stmt
            .query_map(params![monitor_id, limit, offset], |row| {
                Ok(CheckResult {
                    id: row.get(0)?,
                    monitor_id: row.get(1)?,
                    status: row.get(2)?,
                    status_code: row.get(3)?,
                    response_time_ms: row.get::<_, i64>(4)? as u64,
                    error_message: row.get(5)?,
                    checked_at: row.get(6)?,
                })
            })
            .context("Failed to query checks")?;

        let mut result = Vec::new();
        for c in checks {
            result.push(c?);
        }
        Ok(result)
    }

    pub async fn get_latest_check(&self, monitor_id: &str) -> Result<Option<CheckResult>> {
        let conn = self.conn.lock().await;
        let check = conn
            .query_row(
                "SELECT id, monitor_id, status, status_code, response_time_ms, error_message, checked_at \
                 FROM checks WHERE monitor_id=?1 ORDER BY checked_at DESC LIMIT 1",
                params![monitor_id],
                |row| {
                    Ok(CheckResult {
                        id: row.get(0)?,
                        monitor_id: row.get(1)?,
                        status: row.get(2)?,
                        status_code: row.get(3)?,
                        response_time_ms: row.get::<_, i64>(4)? as u64,
                        error_message: row.get(5)?,
                        checked_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("Failed to query latest check")?;
        Ok(check)
    }

    pub async fn get_timeline(
        &self,
        monitor_id: &str,
        since: &str,
    ) -> Result<Vec<crate::models::TimelinePoint>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT checked_at, status, response_time_ms FROM checks \
                 WHERE monitor_id=?1 AND checked_at>=?2 ORDER BY checked_at ASC",
            )
            .context("Failed to prepare get_timeline")?;

        let points = stmt
            .query_map(params![monitor_id, since], |row| {
                Ok(crate::models::TimelinePoint {
                    checked_at: row.get(0)?,
                    status: row.get(1)?,
                    response_time_ms: Some(row.get::<_, i64>(2)? as u64),
                })
            })
            .context("Failed to query timeline")?;

        let mut result = Vec::new();
        for p in points {
            result.push(p?);
        }
        Ok(result)
    }

    pub async fn get_recent_checks_global(
        &self,
        limit: i64,
    ) -> Result<Vec<CheckResult>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.monitor_id, c.status, c.status_code, c.response_time_ms, c.error_message, c.checked_at \
                 FROM checks c INNER JOIN (SELECT monitor_id, MAX(checked_at) as max_checked FROM checks GROUP BY monitor_id) latest \
                 ON c.monitor_id = latest.monitor_id AND c.checked_at = latest.max_checked \
                 ORDER BY c.checked_at DESC LIMIT ?1",
            )
            .context("Failed to prepare get_recent_checks")?;

        let checks = stmt
            .query_map(params![limit], |row| {
                Ok(CheckResult {
                    id: row.get(0)?,
                    monitor_id: row.get(1)?,
                    status: row.get(2)?,
                    status_code: row.get(3)?,
                    response_time_ms: row.get::<_, i64>(4)? as u64,
                    error_message: row.get(5)?,
                    checked_at: row.get(6)?,
                })
            })
            .context("Failed to query recent checks")?;

        let mut result = Vec::new();
        for c in checks {
            result.push(c?);
        }
        Ok(result)
    }

    pub async fn get_monitor_summaries(&self) -> Result<Vec<crate::models::MonitorSummary>> {
        let monitors = self.list_monitors().await?;
        let mut summaries = Vec::new();

        for m in monitors {
            let latest = self.get_latest_check(&m.id).await?;
            let uptime_7d = self.calculate_uptime(&m.id, 7).await?;
            let uptime_30d = self.calculate_uptime(&m.id, 30).await?;

            summaries.push(crate::models::MonitorSummary {
                id: m.id,
                name: m.name,
                monitor_type: m.monitor_type,
                target: m.target,
                enabled: m.enabled,
                last_status: latest.as_ref().map(|c| c.status.clone()),
                last_response_time_ms: latest.as_ref().map(|c| c.response_time_ms),
                last_checked_at: latest.map(|c| c.checked_at),
                uptime_7d,
                uptime_30d,
            });
        }
        Ok(summaries)
    }

    async fn calculate_uptime(&self, monitor_id: &str, days: i64) -> Result<Option<f64>> {
        let conn = self.conn.lock().await;
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();

        let result = conn
            .query_row(
                "SELECT \
                 COUNT(*) as total, \
                 SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) as up_count \
                 FROM checks WHERE monitor_id=?1 AND checked_at>=?2",
                params![monitor_id, cutoff],
                |row| {
                    let total: i64 = row.get(0)?;
                    let up: i64 = row.get(1)?;
                    Ok((total, up))
                },
            )
            .optional()
            .context("Failed to calculate uptime")?;

        match result {
            Some((total, up)) if total > 0 => Ok(Some(up as f64 / total as f64 * 100.0)),
            _ => Ok(None),
        }
    }

    pub async fn get_dashboard_status(&self) -> Result<crate::models::DashboardStatus> {
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
                total_rt += latest.response_time_ms;
                rt_count += 1;
            }
        }

        let cutoff_24h = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let conn = self.conn.lock().await;
        let checks_24h: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM checks WHERE checked_at>=?1",
                params![cutoff_24h],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(crate::models::DashboardStatus {
            total_monitors: total,
            enabled_monitors: enabled,
            up_monitors: up,
            down_monitors: down,
            total_checks_24h: checks_24h as u64,
            avg_response_time_24h: if rt_count > 0 {
                Some(total_rt / rt_count)
            } else {
                None
            },
        })
    }

    pub async fn cleanup_old_checks(&self, retention_days: i64) -> Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM checks WHERE checked_at<?1",
            params![cutoff],
        )
        .context("Failed to cleanup old checks")?;
        Ok(())
    }

    // ───── Notifiers ─────

    pub async fn list_notifiers(&self) -> Result<Vec<Notifier>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, notifier_type, config_json, enabled, created_at, updated_at \
                 FROM notifiers ORDER BY name",
            )
            .context("Failed to prepare list_notifiers")?;

        let notifiers = stmt
            .query_map([], |row| {
                let config_json: String = row.get(3)?;
                Ok(Notifier {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    notifier_type: row.get(2)?,
                    config_json: serde_json::from_str(&config_json)
                        .unwrap_or(serde_json::Value::Object(Default::default())),
                    enabled: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .context("Failed to query notifiers")?;

        let mut result = Vec::new();
        for n in notifiers {
            result.push(n?);
        }
        Ok(result)
    }

    pub async fn get_notifier(&self, id: &str) -> Result<Option<Notifier>> {
        let conn = self.conn.lock().await;
        let notifier = conn
            .query_row(
                "SELECT id, name, notifier_type, config_json, enabled, created_at, updated_at \
                 FROM notifiers WHERE id=?1",
                params![id],
                |row| {
                    let config_json: String = row.get(3)?;
                    Ok(Notifier {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        notifier_type: row.get(2)?,
                        config_json: serde_json::from_str(&config_json)
                            .unwrap_or(serde_json::Value::Object(Default::default())),
                        enabled: row.get::<_, i32>(4)? != 0,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .context("Failed to query notifier")?;
        Ok(notifier)
    }

    pub async fn upsert_notifier(&self, id: &str, notifier: &Notifier) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let config_json =
            serde_json::to_string(&notifier.config_json).context("Failed to serialize config")?;

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO notifiers (id, name, notifier_type, config_json, enabled, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
             name=excluded.name, notifier_type=excluded.notifier_type, config_json=excluded.config_json, \
             enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                id,
                notifier.name,
                notifier.notifier_type,
                config_json,
                notifier.enabled as i32,
                now,
                now,
            ],
        )
        .context("Failed to upsert notifier")?;
        Ok(())
    }

    pub async fn delete_notifier(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn
            .execute("DELETE FROM notifiers WHERE id=?1", params![id])
            .context("Failed to delete notifier")?;
        Ok(rows > 0)
    }

    // ───── Settings ─────

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to get setting")?;
        Ok(value)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .context("Failed to set setting")?;
        Ok(())
    }
}