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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
        };
        db.insert_check(&check).await.unwrap();
    }
    let summaries = db.get_monitor_summaries().await.unwrap();
    let uptime = summaries[0].uptime_7d.unwrap();
    assert!((uptime - 80.0).abs() < 0.01);
}

/// Build a check for the given monitor, status and timestamp.
fn check_at(monitor_id: &str, status: &str, checked_at: &str) -> CheckResult {
    CheckResult {
        id: 0,
        monitor_id: monitor_id.into(),
        status: status.into(),
        status_code: None,
        response_time_ms: 10,
        error_message: None,
        checked_at: checked_at.into(),
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
    }
}

/// Insert `count` checks with the given status, one minute apart, all inside the
/// 7-day window.
async fn insert_checks(db: &Database, monitor_id: &str, status: &str, count: i64) {
    let now = chrono::Utc::now();
    for i in 0..count {
        let checked_at = (now - chrono::Duration::minutes(i)).to_rfc3339();
        let check = check_at(monitor_id, status, &checked_at);
        db.insert_check(&check).await.unwrap();
    }
}

/// Insert `count` checks sharing the given timestamp, so they all land in a
/// single timeline bucket.
async fn insert_checks_at(db: &Database, monitor_id: &str, status: &str, count: i64, at: &str) {
    for _ in 0..count {
        let check = check_at(monitor_id, status, at);
        db.insert_check(&check).await.unwrap();
    }
}

/// Scenario: indeterminate no penaliza el uptime.
#[tokio::test]
async fn indeterminate_checks_should_not_penalize_uptime() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-indet")).await.unwrap();
    insert_checks(&db, "m-indet", "up", 8).await;
    insert_checks(&db, "m-indet", "indeterminate", 2).await;

    let summaries = db.get_monitor_summaries().await.unwrap();
    let uptime = summaries[0].uptime_7d.unwrap();
    assert!(
        (uptime - 100.0).abs() < 0.01,
        "8 up + 2 indeterminate must be 100%, got {uptime}"
    );
}

/// Scenario: down sí penaliza el uptime.
#[tokio::test]
async fn down_checks_should_still_penalize_uptime() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-down")).await.unwrap();
    insert_checks(&db, "m-down", "up", 8).await;
    insert_checks(&db, "m-down", "down", 2).await;

    let summaries = db.get_monitor_summaries().await.unwrap();
    let uptime = summaries[0].uptime_7d.unwrap();
    assert!(
        (uptime - 80.0).abs() < 0.01,
        "8 up + 2 down must be 80%, got {uptime}"
    );
}

/// Scenario: ventana solo con indeterminate.
#[tokio::test]
async fn a_window_with_only_indeterminate_checks_has_no_uptime() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-only-indet"))
        .await
        .unwrap();
    insert_checks(&db, "m-only-indet", "indeterminate", 3).await;

    let summaries = db.get_monitor_summaries().await.unwrap();
    assert!(
        summaries[0].uptime_7d.is_none(),
        "a window without attributable checks has no uptime"
    );
}

/// The batched uptime path (monitor list) applies the same exclusion.
#[tokio::test]
async fn batch_uptime_should_also_exclude_indeterminate_checks() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-batch")).await.unwrap();
    insert_checks(&db, "m-batch", "up", 8).await;
    insert_checks(&db, "m-batch", "indeterminate", 2).await;

    let (_monitors, _total, summaries) = db
        .list_monitors_paginated(1, 10, None, None, None)
        .await
        .unwrap();
    let uptime = summaries[0].uptime_7d.unwrap();
    assert!(
        (uptime - 100.0).abs() < 0.01,
        "batched uptime must exclude indeterminate, got {uptime}"
    );
}

/// The dashboard must not report a monitor as down because of an unattributable
/// failure.
#[tokio::test]
async fn dashboard_should_not_count_an_indeterminate_check_as_down() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-dash-indet"))
        .await
        .unwrap();
    insert_checks(&db, "m-dash-indet", "indeterminate", 1).await;

    let status = db.get_dashboard_status().await.unwrap();
    assert_eq!(status.up_monitors, 0);
    assert_eq!(status.down_monitors, 0);
    assert_eq!(status.indeterminate_monitors, 1);
}

/// Scenario M3: el dashboard expone el recuento de monitores indeterminate y el
/// total cuadra con la suma de categorías.
#[tokio::test]
async fn dashboard_counts_indeterminate_monitors_separately() {
    let (db, _dir) = setup_db().await;
    for id in ["m-d-up", "m-d-down", "m-d-indet"] {
        db.create_monitor(&sample_monitor(id)).await.unwrap();
    }
    let now = chrono::Utc::now().to_rfc3339();
    db.insert_check(&check_at("m-d-up", "up", &now))
        .await
        .unwrap();
    db.insert_check(&check_at("m-d-down", "down", &now))
        .await
        .unwrap();
    db.insert_check(&check_at("m-d-indet", "indeterminate", &now))
        .await
        .unwrap();

    let status = db.get_dashboard_status().await.unwrap();
    assert_eq!(status.up_monitors, 1);
    assert_eq!(status.down_monitors, 1);
    assert_eq!(status.indeterminate_monitors, 1);
    assert_eq!(
        status.total_monitors,
        status.up_monitors + status.down_monitors + status.indeterminate_monitors,
        "the total must add up to the sum of the categories"
    );
}

// ── M2: el timeline no pinta como down un bucket sin fallos atribuibles ──

/// Scenario: bucket cuyos únicos checks son `indeterminate`.
#[tokio::test]
async fn a_timeline_bucket_with_only_indeterminate_checks_is_not_down() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-tl-only-indet"))
        .await
        .unwrap();
    let at = chrono::Utc::now().to_rfc3339();
    insert_checks_at(&db, "m-tl-only-indet", "indeterminate", 3, &at).await;

    let since = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let buckets = db
        .get_timeline_buckets("m-tl-only-indet", &since, 60)
        .await
        .unwrap();

    assert_eq!(buckets.len(), 1, "every check shares one bucket");
    assert_eq!(buckets[0].dominant_status, "indeterminate");
    assert_ne!(buckets[0].dominant_status, "down");
}

/// Scenario: bucket con up e indeterminate.
#[tokio::test]
async fn a_timeline_bucket_excludes_indeterminate_from_the_uptime_percentage() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-tl-mixed"))
        .await
        .unwrap();
    let at = chrono::Utc::now().to_rfc3339();
    insert_checks_at(&db, "m-tl-mixed", "up", 1, &at).await;
    insert_checks_at(&db, "m-tl-mixed", "indeterminate", 1, &at).await;

    let since = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let buckets = db
        .get_timeline_buckets("m-tl-mixed", &since, 60)
        .await
        .unwrap();

    assert_eq!(buckets.len(), 1);
    assert!(
        (buckets[0].up_pct - 100.0).abs() < 0.01,
        "the indeterminate check must leave the denominator, got {}",
        buckets[0].up_pct
    );
    assert_eq!(
        buckets[0].dominant_status, "up",
        "with attributable checks the dominant status stays binary"
    );
    assert_eq!(buckets[0].count, 2, "count still reports every check");
}

/// Scenario: bucket con caída real.
#[tokio::test]
async fn a_timeline_bucket_with_a_real_outage_is_down() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-tl-down"))
        .await
        .unwrap();
    let at = chrono::Utc::now().to_rfc3339();
    insert_checks_at(&db, "m-tl-down", "down", 3, &at).await;
    insert_checks_at(&db, "m-tl-down", "up", 1, &at).await;

    let since = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let buckets = db
        .get_timeline_buckets("m-tl-down", &since, 60)
        .await
        .unwrap();

    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].dominant_status, "down");
    assert!(
        (buckets[0].up_pct - 25.0).abs() < 0.01,
        "one up out of four attributable checks is 25%, got {}",
        buckets[0].up_pct
    );
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
            tls_cert_expires_at: None,
            tls_cert_days_left: None,
            error_kind: None,
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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
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

// ───── API Keys (feature: api-keys) ─────
// RED: los métodos de `Database` aún son stubs que devuelven `Err`, por lo que
// estos tests fallan en runtime (RED) a la espera del GREEN phase.

#[tokio::test]
async fn test_create_and_list_api_keys() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap();
    let keys = db.list_api_keys().await.unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].name, "ci-cd");
    assert_eq!(
        keys[0].api_key, "550e8400-e29b-41d4-a716-446655440000",
        "el api_key UUID generado debe conservarse"
    );
}

#[tokio::test]
async fn test_create_api_key_duplicate_name_rejected() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "key-1").await.unwrap();
    // Segundo key con el mismo `name` → debe fallar por unicidad
    let res = db.create_api_key("ak-2", "ci-cd", "key-2").await;
    assert!(
        res.is_err(),
        "un name duplicado debe generar un error de unicidad"
    );
    // La lista debe seguir teniendo solo la primera key
    assert_eq!(db.list_api_keys().await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_create_api_key_duplicate_api_key_rejected() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "first", "dup-key").await.unwrap();
    // Segundo key con el mismo `api_key` → debe fallar por unicidad
    let res = db.create_api_key("ak-2", "second", "dup-key").await;
    assert!(
        res.is_err(),
        "un api_key duplicado debe generar un error de unicidad"
    );
    assert_eq!(db.list_api_keys().await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_update_api_key_name_preserves_secret() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "secret-key")
        .await
        .unwrap();
    assert!(db.update_api_key_name("ak-1", "renamed").await.unwrap());
    let key = db
        .get_api_key_by_value("secret-key")
        .await
        .unwrap()
        .expect("should exist");
    assert_eq!(key.name, "renamed");
    assert_eq!(
        key.api_key, "secret-key",
        "el api_key UUID NO debe cambiar al editar solo el nombre"
    );
}

#[tokio::test]
async fn test_update_api_key_name_duplicate_rejected() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "key-1").await.unwrap();
    db.create_api_key("ak-2", "deploy", "key-2").await.unwrap();
    // Renombrar ak-1 a un name ya usado por ak-2 → debe fallar
    let res = db.update_api_key_name("ak-1", "deploy").await;
    assert!(res.is_err(), "renombrar a un name duplicado debe fallar");
}

#[tokio::test]
async fn test_update_api_key_name_nonexistent() {
    let (db, _dir) = setup_db().await;
    let res = db.update_api_key_name("no-such-id", "whatever").await;
    assert!(
        res.is_err() || !res.unwrap(),
        "un id inexistente debe no-error o devolver false"
    );
}

#[tokio::test]
async fn test_delete_api_key() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "secret-key")
        .await
        .unwrap();
    assert!(db.delete_api_key("ak-1").await.unwrap());
    assert_eq!(db.list_api_keys().await.unwrap().len(), 0);
}

#[tokio::test]
async fn test_get_api_key_by_value() {
    let (db, _dir) = setup_db().await;
    db.create_api_key("ak-1", "ci-cd", "550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap();
    let key = db
        .get_api_key_by_value("550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap()
        .expect("debe devolver la key buscada");
    assert_eq!(key.id, "ak-1");
    assert_eq!(key.name, "ci-cd");
}

#[tokio::test]
async fn test_get_api_key_by_value_missing() {
    let (db, _dir) = setup_db().await;
    assert!(
        db.get_api_key_by_value("not-present")
            .await
            .unwrap()
            .is_none(),
        "un api_key inexistente debe devolver None"
    );
}

#[tokio::test]
async fn test_api_key_name_unique() {
    let (db, _dir) = setup_db().await;
    // Nombre libre → true
    assert!(db.api_key_name_unique("free-name", None).await.unwrap());
    db.create_api_key("ak-1", "ci-cd", "secret-key")
        .await
        .unwrap();
    // Nombre duplicado (sin exclusion) → false
    assert!(!db.api_key_name_unique("ci-cd", None).await.unwrap());
    // Excluyendo el propio id → true (no se cuenta a sí mismo)
    assert!(db.api_key_name_unique("ci-cd", Some("ak-1")).await.unwrap());
}

// ───── Failure cause persistence (Fase 2: error_kind) ─────

/// Columns of the `checks` table, read through a separate connection so the
/// assertion is about the schema itself and not about what the code exposes.
async fn checks_columns(path: &std::path::Path) -> Vec<String> {
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(path);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('checks') ORDER BY cid")
            .fetch_all(&pool)
            .await
            .unwrap();

    pool.close().await;
    rows.into_iter().map(|(_, name)| name).collect()
}

fn sample_check(monitor_id: &str) -> CheckResult {
    CheckResult {
        id: 0,
        monitor_id: monitor_id.into(),
        status: "down".into(),
        status_code: None,
        response_time_ms: 5,
        error_message: Some("TCP connection failed".into()),
        checked_at: chrono::Utc::now().to_rfc3339(),
        tls_cert_expires_at: None,
        tls_cert_days_left: None,
        error_kind: None,
    }
}

/// Scenario: Migración idempotente.
#[tokio::test]
async fn test_error_kind_migration_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy.db");

    // A database predating the column: same `checks` table, without `error_kind`.
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            monitor_id TEXT NOT NULL,
            status TEXT NOT NULL, status_code INTEGER,
            response_time_ms INTEGER NOT NULL DEFAULT 0, error_message TEXT,
            checked_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    assert!(
        !checks_columns(&path)
            .await
            .contains(&"error_kind".to_string()),
        "the fixture must start without error_kind"
    );

    // First initialisation adds the column…
    let db = Database::open(&path).await.unwrap();
    assert!(
        checks_columns(&path)
            .await
            .contains(&"error_kind".to_string()),
        "opening a legacy DB must add checks.error_kind"
    );
    drop(db);

    // …and a second one must not fail.
    let db = Database::open(&path).await.unwrap();
    assert!(checks_columns(&path)
        .await
        .contains(&"error_kind".to_string()));
    drop(db);
}

/// Scenario: Persistir la causa de un fallo.
#[tokio::test]
async fn test_insert_check_persists_error_kind() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-kind")).await.unwrap();

    let check = CheckResult {
        error_kind: Some("connect_refused".into()),
        ..sample_check("m-kind")
    };
    db.insert_check(&check).await.unwrap();

    let latest = db
        .get_latest_check("m-kind")
        .await
        .unwrap()
        .expect("the inserted check must be readable");
    assert_eq!(latest.error_kind.as_deref(), Some("connect_refused"));

    let listed = db.get_checks("m-kind", 10, 0).await.unwrap();
    assert_eq!(listed[0].error_kind.as_deref(), Some("connect_refused"));
}

/// Scenario: Check correcto sin causa.
#[tokio::test]
async fn test_insert_check_stores_null_error_kind_without_cause() {
    let (db, dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-ok")).await.unwrap();

    let mut check = sample_check("m-ok");
    check.status = "up".into();
    check.error_message = None;
    db.insert_check(&check).await.unwrap();

    let latest = db
        .get_latest_check("m-ok")
        .await
        .unwrap()
        .expect("the inserted check must be readable");
    assert_eq!(latest.error_kind, None);

    // NULL in the row itself, not an empty string.
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("test.db"));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let (kind,): (Option<String>,) =
        sqlx::query_as("SELECT error_kind FROM checks WHERE monitor_id='m-ok'")
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    assert_eq!(kind, None);
}

/// Scenario: Lectura de la causa.
#[tokio::test]
async fn test_error_kind_is_visible_through_filtered_and_recent_reads() {
    let (db, _dir) = setup_db().await;
    db.create_monitor(&sample_monitor("m-read")).await.unwrap();

    let check = CheckResult {
        error_kind: Some("dns_servfail".into()),
        ..sample_check("m-read")
    };
    db.insert_check(&check).await.unwrap();

    let filtered = db
        .get_checks_filtered("m-read", 10, 0, &["down"])
        .await
        .unwrap();
    assert_eq!(filtered[0].error_kind.as_deref(), Some("dns_servfail"));

    let recent = db.get_recent_checks_global(10).await.unwrap();
    assert_eq!(recent[0].error_kind.as_deref(), Some("dns_servfail"));
}

/// Scenario: un latido registra un check sin causa (Mn3).
#[tokio::test]
async fn test_heartbeat_pulse_records_check_without_error_kind() {
    let (db, _dir) = setup_db().await;
    let monitor = Monitor {
        monitor_type: "heartbeat".into(),
        token: Some("hb-token".into()),
        ..sample_monitor("m-hb")
    };
    db.create_monitor(&monitor).await.unwrap();

    let recorded = db
        .record_heartbeat_pulse("hb-token")
        .await
        .unwrap()
        .expect("the token must identify the monitor");
    assert_eq!(recorded.id, "m-hb");

    let latest = db
        .get_latest_check("m-hb")
        .await
        .unwrap()
        .expect("the pulse must record a check");
    assert_eq!(latest.status, "up");
    assert_eq!(latest.error_kind, None, "a heartbeat has no failure cause");
}
