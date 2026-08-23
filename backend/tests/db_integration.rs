use std::path::PathBuf;
use tempfile::TempDir;
use watchbeat::db::Database;
use watchbeat::models::{CheckResult, Monitor, Notifier};

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
        tags: vec![],
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

#[tokio::test]
async fn test_create_and_list_monitors() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-1")).await.unwrap();
    let monitors = db.list_monitors().await.unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id, "m-1");
}

#[tokio::test]
async fn test_get_monitor() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-2")).await.unwrap();
    let m = db.get_monitor("m-2").await.unwrap().expect("should exist");
    assert_eq!(m.name, "Monitor m-2");
}

#[tokio::test]
async fn test_get_nonexistent_monitor() {
    let (db, _dir) = setup_db().await;
    assert!(db.get_monitor("no-exist").await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_monitor() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-3")).await.unwrap();
    let mut m = sample_monitor("m-3");
    m.name = "Updated".into();
    m.enabled = false;
    assert!(db.update_monitor("m-3", &m).await.unwrap());
    let updated = db.get_monitor("m-3").await.unwrap().unwrap();
    assert_eq!(updated.name, "Updated");
    assert!(!updated.enabled);
}

#[tokio::test]
async fn test_delete_monitor() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-4")).await.unwrap();
    assert!(db.delete_monitor("m-4").await.unwrap());
    assert!(db.get_monitor("m-4").await.unwrap().is_none());
}

#[tokio::test]
async fn test_toggle_monitor() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-5")).await.unwrap();
    assert_eq!(db.toggle_monitor("m-5").await.unwrap(), Some(false));
    assert_eq!(db.toggle_monitor("m-5").await.unwrap(), Some(true));
}

#[tokio::test]
async fn test_toggle_nonexistent() {
    let (db, _dir) = setup_db().await;
    assert!(db.toggle_monitor("nope").await.unwrap().is_none());
}

#[tokio::test]
async fn test_insert_and_get_checks() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-c1")).await.unwrap();
    let check = CheckResult {
        id: 0,
        monitor_id: "m-c1".into(),
        status: "up".into(),
        status_code: Some(200),
        response_time_ms: 42,
        error_message: None,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    let id = db.insert_check(&check).await.unwrap();
    assert!(id > 0);
    let checks = db.get_checks("m-c1", 10, 0).await.unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, "up");
}

#[tokio::test]
async fn test_get_latest_check() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-c2")).await.unwrap();
    assert!(db.get_latest_check("m-c2").await.unwrap().is_none());
    for i in 0..3 {
        let check = CheckResult {
            id: 0,
            monitor_id: "m-c2".into(),
            status: if i == 2 { "down".into() } else { "up".into() },
            status_code: None,
            response_time_ms: 10 * i,
            error_message: None,
            checked_at: chrono::Utc::now().to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    let latest = db.get_latest_check("m-c2").await.unwrap().unwrap();
    assert_eq!(latest.status, "down");
}

#[tokio::test]
async fn test_notifier_crud() {
    let (db, _dir) = setup_db().await;
    let n = Notifier {
        id: "n-1".into(),
        name: "Telegram".into(),
        notifier_type: "telegram".into(),
        config_json: serde_json::json!({"bot_token": "abc"}),
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
    };
    db.upsert_notifier("n-1", &n).await.unwrap();
    assert_eq!(db.list_notifiers().await.unwrap().len(), 1);
    let fetched = db.get_notifier("n-1").await.unwrap().unwrap();
    assert_eq!(fetched.name, "Telegram");
    assert!(db.delete_notifier("n-1").await.unwrap());
    assert!(db.get_notifier("n-1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_settings() {
    let (db, _dir) = setup_db().await;
    assert!(db.get_setting("key1").await.unwrap().is_none());
    db.set_setting("key1", "val1").await.unwrap();
    assert_eq!(db.get_setting("key1").await.unwrap(), Some("val1".into()));
    db.set_setting("key1", "val2").await.unwrap();
    assert_eq!(db.get_setting("key1").await.unwrap(), Some("val2".into()));
}

#[tokio::test]
async fn test_dashboard_status() {
    let (db, _dir) = setup_db().await;
    let status = db.get_dashboard_status().await.unwrap();
    assert_eq!(status.total_monitors, 0);
    db.create_monitor(&sample_monitor("m-d1")).await.unwrap();
    let check = CheckResult {
        id: 0,
        monitor_id: "m-d1".into(),
        status: "up".into(),
        status_code: Some(200),
        response_time_ms: 50,
        error_message: None,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    db.insert_check(&check).await.unwrap();
    let status = db.get_dashboard_status().await.unwrap();
    assert_eq!(status.total_monitors, 1);
    assert_eq!(status.up_monitors, 1);
    assert!(status.avg_response_time_24h.is_some());
}

#[tokio::test]
async fn test_calculate_uptime() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-u1")).await.unwrap();
    let now = chrono::Utc::now();
    for i in 0..10 {
        let status = if i < 8 { "up" } else { "down" };
        let check = CheckResult {
            id: 0,
            monitor_id: "m-u1".into(),
            status: status.into(),
            status_code: None,
            response_time_ms: 10,
            error_message: None,
            checked_at: (now - chrono::Duration::hours(i)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    let summaries = db.get_monitor_summaries().await.unwrap();
    let uptime = summaries[0].uptime_7d.unwrap();
    assert!((uptime - 80.0).abs() < 0.01);
}

#[tokio::test]
async fn test_set_failed_checks() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-fc")).await.unwrap();
    db.set_failed_checks("m-fc", 3).await.unwrap();
    let m = db.get_monitor("m-fc").await.unwrap().unwrap();
    assert_eq!(m.failed_checks, 3);
}

#[tokio::test]
async fn test_reset_failed_checks() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-rc")).await.unwrap();
    db.set_failed_checks("m-rc", 5).await.unwrap();
    db.reset_failed_checks("m-rc").await.unwrap();
    let m = db.get_monitor("m-rc").await.unwrap().unwrap();
    assert_eq!(m.failed_checks, 0);
}
