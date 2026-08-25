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

#[tokio::test]
async fn test_get_timeline() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-tl")).await.unwrap();
    let now = chrono::Utc::now();
    for i in 0..5 {
        let status = match i {
            0 => "up",
            1 => "down",
            2 => "up",
            3 => "error",
            4 => "up",
            _ => unreachable!(),
        };
        let check = CheckResult {
            id: 0,
            monitor_id: "m-tl".into(),
            status: status.into(),
            status_code: Some(200),
            response_time_ms: 10 * (i + 1),
            error_message: if i == 3 { Some("timeout".into()) } else { None },
            checked_at: (now - chrono::Duration::hours(4 - i)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    // since covers all 5 checks (6 hours ago)
    let since = (now - chrono::Duration::hours(6)).to_rfc3339();
    let timeline = db.get_timeline("m-tl", &since).await.unwrap();
    assert_eq!(timeline.len(), 5);
    // Timeline should be in ASC order (oldest first)
    for i in 0..4 {
        assert!(
            timeline[i].checked_at <= timeline[i + 1].checked_at,
            "timeline should be in ascending order"
        );
    }
    assert_eq!(timeline[0].status, "up");
    assert_eq!(timeline[1].status, "down");
    assert_eq!(timeline[2].status, "up");
    assert_eq!(timeline[3].status, "error");
    assert_eq!(timeline[4].status, "up");
}

#[tokio::test]
async fn test_get_timeline_since() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-tls")).await.unwrap();
    let now = chrono::Utc::now();
    // Insert 3 old checks (10, 8, 6 hours ago)
    for i in (0..3).rev() {
        let check = CheckResult {
            id: 0,
            monitor_id: "m-tls".into(),
            status: "up".into(),
            status_code: None,
            response_time_ms: 10,
            error_message: None,
            checked_at: (now - chrono::Duration::hours(10 + i * 2)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    // Insert 2 recent checks (1 and 0 hours ago)
    for i in 0..2 {
        let check = CheckResult {
            id: 0,
            monitor_id: "m-tls".into(),
            status: "down".into(),
            status_code: None,
            response_time_ms: 20,
            error_message: None,
            checked_at: (now - chrono::Duration::hours(i)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    // since = 5 hours ago → should only return the 2 recent checks
    let since = (now - chrono::Duration::hours(5)).to_rfc3339();
    let timeline = db.get_timeline("m-tls", &since).await.unwrap();
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].status, "down");
    assert_eq!(timeline[1].status, "down");
}

#[tokio::test]
async fn test_get_monitor_summaries() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-ms1")).await.unwrap();
    db.create_monitor(&sample_monitor("m-ms2")).await.unwrap();
    let now = chrono::Utc::now();
    // Insert checks for m-ms1: 8 up, 2 down → 80% uptime
    for i in 0..10 {
        let status = if i < 8 { "up" } else { "down" };
        let check = CheckResult {
            id: 0,
            monitor_id: "m-ms1".into(),
            status: status.into(),
            status_code: None,
            response_time_ms: 10,
            error_message: None,
            checked_at: (now - chrono::Duration::hours(i)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    // Insert checks for m-ms2: 5 up, 5 down → 50% uptime
    for i in 0..10 {
        let status = if i < 5 { "up" } else { "down" };
        let check = CheckResult {
            id: 0,
            monitor_id: "m-ms2".into(),
            status: status.into(),
            status_code: None,
            response_time_ms: 20,
            error_message: None,
            checked_at: (now - chrono::Duration::hours(i)).to_rfc3339(),
        };
        db.insert_check(&check).await.unwrap();
    }
    let summaries = db.get_monitor_summaries().await.unwrap();
    assert_eq!(summaries.len(), 2);
    let s1 = summaries.iter().find(|s| s.id == "m-ms1").unwrap();
    let s2 = summaries.iter().find(|s| s.id == "m-ms2").unwrap();
    assert!((s1.uptime_7d.unwrap() - 80.0).abs() < 0.01);
    assert!((s2.uptime_7d.unwrap() - 50.0).abs() < 0.01);
    // Latest check (i=0) has status "up" for m-ms1, "up" for m-ms2
    assert!(s1.last_status.as_deref() == Some("up"));
    assert!(s2.last_status.as_deref() == Some("up"));
}

#[tokio::test]
async fn test_calculate_uptime_50_percent() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-u50")).await.unwrap();
    let now = chrono::Utc::now();
    // 5 up + 5 down = 50% uptime
    for i in 0..10 {
        let status = if i < 5 { "up" } else { "down" };
        let check = CheckResult {
            id: 0,
            monitor_id: "m-u50".into(),
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
    assert!((uptime - 50.0).abs() < 0.01);
}

#[tokio::test]
async fn test_get_dashboard_status_with_mixed() {
    let (db, _dir) = setup_db().await;
    let now = chrono::Utc::now();
    // Monitor 1: up
    db.create_monitor(&sample_monitor("m-ds1")).await.unwrap();
    let check = CheckResult {
        id: 0,
        monitor_id: "m-ds1".into(),
        status: "up".into(),
        status_code: Some(200),
        response_time_ms: 50,
        error_message: None,
        checked_at: now.to_rfc3339(),
    };
    db.insert_check(&check).await.unwrap();
    // Monitor 2: down
    let mut m2 = sample_monitor("m-ds2");
    m2.enabled = true;
    db.create_monitor(&m2).await.unwrap();
    let check = CheckResult {
        id: 0,
        monitor_id: "m-ds2".into(),
        status: "down".into(),
        status_code: None,
        response_time_ms: 1000,
        error_message: Some("timeout".into()),
        checked_at: now.to_rfc3339(),
    };
    db.insert_check(&check).await.unwrap();
    // Monitor 3: disabled (no checks)
    let mut m3 = sample_monitor("m-ds3");
    m3.enabled = false;
    db.create_monitor(&m3).await.unwrap();
    let status = db.get_dashboard_status().await.unwrap();
    assert_eq!(status.total_monitors, 3);
    assert_eq!(status.enabled_monitors, 2);
    assert_eq!(status.up_monitors, 1);
    assert_eq!(status.down_monitors, 1);
    assert!(status.avg_response_time_24h.is_some());
    // avg = (50 + 1000) / 2 = 525
    assert_eq!(status.avg_response_time_24h.unwrap(), 525);
}

#[tokio::test]
async fn test_cleanup_old_checks() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-cl")).await.unwrap();
    let now = chrono::Utc::now();
    // Insert a check from 10 days ago
    let old_check = CheckResult {
        id: 0,
        monitor_id: "m-cl".into(),
        status: "up".into(),
        status_code: None,
        response_time_ms: 10,
        error_message: None,
        checked_at: (now - chrono::Duration::days(10)).to_rfc3339(),
    };
    db.insert_check(&old_check).await.unwrap();
    // Insert a recent check
    let recent_check = CheckResult {
        id: 0,
        monitor_id: "m-cl".into(),
        status: "down".into(),
        status_code: None,
        response_time_ms: 20,
        error_message: None,
        checked_at: now.to_rfc3339(),
    };
    db.insert_check(&recent_check).await.unwrap();
    // Verify both exist before cleanup
    let checks_before = db.get_checks("m-cl", 100, 0).await.unwrap();
    assert_eq!(checks_before.len(), 2);
    // Cleanup with retention of 7 days → old check should be deleted
    db.cleanup_old_checks(7).await.unwrap();
    let checks_after = db.get_checks("m-cl", 100, 0).await.unwrap();
    assert_eq!(checks_after.len(), 1);
    assert_eq!(checks_after[0].status, "down");
}
