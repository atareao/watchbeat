//! Query-plan and performance regression tests for the `checks` table (#33).
//!
//! These tests assert the *shape* of the SQLite query plans (deterministic) and
//! a generous performance smoke test on a synthetic dataset. They open a second
//! connection to the same database file to run `EXPLAIN QUERY PLAN` and inspect
//! `sqlite_master` / `sqlite_stat1` without exposing the internal pool.

use std::path::Path;
use std::time::Instant;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};
use tempfile::TempDir;
use watchbeat::db::Database;
use watchbeat::models::Monitor;

fn sample_monitor(id: &str) -> Monitor {
    Monitor {
        id: id.into(),
        name: format!("Monitor {}", id),
        monitor_type: "http".into(),
        target: "https://example.com".into(),
        config_json: serde_json::json!({}),
        interval_seconds: 300,
        timeout_seconds: 30,
        enabled: true,
        notifier_id: None,
        confirmations_required: 0,
        failed_checks: 0,
        latency_threshold_ms: None,
        message_template_down: None,
        message_template_latency: None,
        message_template_up: None,
        message_template_expiry: None,
        tags: vec![],
        token: None,
        grace_seconds: None,
        last_seen_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

async fn setup_db() -> (Database, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let db = Database::open(&path).await.unwrap();
    (db, dir)
}

async fn side_pool(dir: &Path) -> SqlitePool {
    let path = dir.join("test.db");
    let opts = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .busy_timeout(std::time::Duration::from_secs(5));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

async fn explain(pool: &SqlitePool, sql: &str) -> Vec<String> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!("EXPLAIN QUERY PLAN {sql}")))
        .fetch_all(pool)
        .await
        .unwrap();
    rows.iter().map(|r| r.get::<String, _>("detail")).collect()
}

async fn index_names(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master \
         WHERE type='index' AND tbl_name=? AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.iter().map(|r| r.get::<String, _>("name")).collect()
}

async fn stat_rows(pool: &SqlitePool, index: &str) -> i64 {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM sqlite_stat1 WHERE idx=?")
        .bind(index)
        .fetch_one(pool)
        .await
        .unwrap();
    row.get::<i64, _>("n")
}

/// Inserts `n` synthetic checks for `monitor_id` spread over the last 30 days.
async fn seed_checks(pool: &SqlitePool, monitor_id: &str, n: i64) {
    sqlx::query(
        "WITH RECURSIVE seq(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM seq WHERE i < ?) \
         INSERT INTO checks (monitor_id, status, status_code, response_time_ms, checked_at) \
         SELECT ?, \
                CASE WHEN i % 100 < 95 THEN 'up' WHEN i % 100 < 98 THEN 'warning' ELSE 'down' END, \
                200, 50 + (i % 500), \
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-' || (i % 2592000) || ' seconds') \
         FROM seq",
    )
    .bind(n)
    .bind(monitor_id)
    .execute(pool)
    .await
    .unwrap();
}

const TIMELINE_SQL: &str = "SELECT \
     strftime('%Y-%m-%dT%H:%M:%SZ', \
           CAST((CAST(strftime('%s', checked_at) / 43200 AS INTEGER) * 43200) AS TEXT), \
     'unixepoch') AS bucket_start, \
     SUM(CASE WHEN status='up' THEN 1 ELSE 0 END), \
     SUM(CASE WHEN status IN ('up','warning','down','error') THEN 1 ELSE 0 END), \
     AVG(CAST(response_time_ms AS FLOAT)), \
     COUNT(*) \
  FROM checks \
  WHERE monitor_id='m1' AND checked_at>=strftime('%Y-%m-%dT%H:%M:%SZ','now','-30 day') \
  GROUP BY bucket_start ORDER BY bucket_start";

const UPTIME_SQL: &str = "SELECT CAST(COUNT(*) AS INTEGER), \
     CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS INTEGER) \
     FROM checks WHERE monitor_id='m1' \
     AND checked_at>=strftime('%Y-%m-%dT%H:%M:%SZ','now','-30 day') \
     AND status IN ('up','warning','down','error')";

#[tokio::test]
async fn timeline_index_exists_and_redundant_indexes_are_gone() {
    let (_db, dir) = setup_db().await;
    let pool = side_pool(dir.path()).await;
    let names = index_names(&pool, "checks").await;

    assert!(
        names.iter().any(|n| n == "idx_checks_timeline"),
        "idx_checks_timeline must exist, found: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "idx_checks_monitor"),
        "redundant idx_checks_monitor must be dropped, found: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "idx_checks_uptime"),
        "redundant idx_checks_uptime must be dropped, found: {names:?}"
    );
}

#[tokio::test]
async fn migration_drops_legacy_indexes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");

    // Create the schema, then simulate a legacy database by recreating the
    // indexes that older versions shipped.
    drop(Database::open(&path).await.unwrap());
    {
        let pool = side_pool(dir.path()).await;
        sqlx::query("CREATE INDEX idx_checks_monitor ON checks(monitor_id, checked_at DESC)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE INDEX idx_checks_uptime ON checks(monitor_id, checked_at, status)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }

    // Reopening must migrate: create the new index and drop the legacy ones.
    let _db = Database::open(&path).await.unwrap();
    let pool = side_pool(dir.path()).await;
    let names = index_names(&pool, "checks").await;

    assert!(
        names.iter().any(|n| n == "idx_checks_timeline"),
        "migration must create idx_checks_timeline, found: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "idx_checks_monitor"),
        "migration must drop idx_checks_monitor, found: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "idx_checks_uptime"),
        "migration must drop idx_checks_uptime, found: {names:?}"
    );
}

#[tokio::test]
async fn prefix_queries_use_timeline_index() {
    let (_db, dir) = setup_db().await;
    let pool = side_pool(dir.path()).await;

    for sql in [
        "SELECT id FROM checks WHERE monitor_id='m1' ORDER BY checked_at DESC LIMIT 50",
        "SELECT id FROM checks WHERE monitor_id='m1' ORDER BY checked_at DESC LIMIT 1",
        "SELECT checked_at, status, response_time_ms FROM checks          WHERE monitor_id='m1' AND checked_at>='2026-01-01T00:00:00Z' ORDER BY checked_at ASC",
    ] {
        let plan = explain(&pool, sql).await.join("\n");
        assert!(
            plan.contains("idx_checks_timeline"),
            "query must use idx_checks_timeline, plan:\n{plan}\nSQL: {sql}"
        );
    }
}

#[tokio::test]
async fn filtered_checks_use_timeline_index_without_sort() {
    let (_db, dir) = setup_db().await;
    let pool = side_pool(dir.path()).await;

    let plan = explain(
        &pool,
        "SELECT id FROM checks WHERE monitor_id='m1' AND status IN ('up','down') \
         ORDER BY checked_at DESC LIMIT 20",
    )
    .await;
    let joined = plan.join("\n");

    assert!(
        joined.contains("idx_checks_timeline"),
        "filtered query must use idx_checks_timeline, plan:\n{joined}"
    );
    assert!(
        !joined.contains("TEMP B-TREE FOR ORDER BY"),
        "filtered query must not sort, plan:\n{joined}"
    );
}

#[tokio::test]
async fn timeline_buckets_use_covering_index() {
    let (_db, dir) = setup_db().await;
    let pool = side_pool(dir.path()).await;

    let plan = explain(&pool, TIMELINE_SQL).await;
    let joined = plan.join("\n");

    assert!(
        joined.contains("COVERING INDEX idx_checks_timeline"),
        "timeline must use idx_checks_timeline as a covering index, plan:\n{joined}"
    );
    assert!(
        !joined.contains("SCAN checks"),
        "timeline must not scan the table, plan:\n{joined}"
    );
}

#[tokio::test]
async fn uptime_uses_covering_index() {
    let (_db, dir) = setup_db().await;
    let pool = side_pool(dir.path()).await;

    let plan = explain(&pool, UPTIME_SQL).await;
    let joined = plan.join("\n");

    assert!(
        joined.contains("COVERING INDEX idx_checks_timeline"),
        "uptime must use idx_checks_timeline as a covering index, plan:\n{joined}"
    );
}

#[tokio::test]
async fn analyze_populates_planner_stats() {
    let (db, dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m1")).await.unwrap();

    // Seed data through a side connection, then reopen the database so that
    // `init_db` runs `ANALYZE` with rows present.
    {
        let pool = side_pool(dir.path()).await;
        seed_checks(&pool, "m1", 1_000).await;
        pool.close().await;
    }
    drop(db);
    let _db = Database::open(&dir.path().join("test.db")).await.unwrap();

    let pool = side_pool(dir.path()).await;
    let n = stat_rows(&pool, "idx_checks_timeline").await;
    assert!(
        n > 0,
        "ANALYZE must populate sqlite_stat1 for idx_checks_timeline"
    );
}

#[tokio::test]
async fn timeline_buckets_are_fast_on_21k_rows() {
    // Real scenario from issue #33: ~21k checks for one monitor over 14 days.
    let (db, dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m1")).await.unwrap();

    let pool = side_pool(dir.path()).await;
    seed_checks(&pool, "m1", 21_000).await;
    // Refresh planner stats after the bulk insert.
    sqlx::query("ANALYZE").execute(&pool).await.unwrap();

    let since = (chrono::Utc::now() - chrono::Duration::days(14)).to_rfc3339();
    // Warm the page cache: the first run pays the cold-read cost.
    let _ = db.get_timeline_buckets("m1", &since, 20_160).await.unwrap();

    let start = Instant::now();
    let buckets = db.get_timeline_buckets("m1", &since, 20_160).await.unwrap();
    let elapsed = start.elapsed();

    assert!(!buckets.is_empty(), "timeline must return buckets");
    // SQLite is bundled and compiled with the Rust profile: debug builds are
    // ~3x slower than release. Keep a wider margin in debug to avoid flakes.
    let budget_ms = if cfg!(debug_assertions) { 250 } else { 100 };
    assert!(
        elapsed.as_millis() < budget_ms,
        "timeline on 21k rows took {elapsed:?}, expected < {budget_ms}ms"
    );
}
