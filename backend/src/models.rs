use serde::{Deserialize, Serialize};

// Re-export types used by publishers
// pub use crate::template::{TemplateContext, TemplateRenderer};

// ───── Monitor ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub monitor_type: String,
    pub target: String,
    pub config_json: serde_json::Value,
    pub interval_seconds: i64,
    pub timeout_seconds: i64,
    pub enabled: bool,
    pub notifier_id: Option<String>,
    pub confirmations_required: i64,
    pub failed_checks: i64,
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
    pub message_template_expiry: Option<String>,
    pub tags: Vec<String>,
    pub token: Option<String>,
    pub grace_seconds: Option<i64>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ───── Check Result ─────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CheckResult {
    pub id: i64,
    pub monitor_id: String,
    pub status: String, // "up" | "down" | "error"
    pub status_code: Option<u16>,
    pub response_time_ms: i64, // SQLite integer
    pub error_message: Option<String>,
    pub checked_at: String,
    pub tls_cert_expires_at: Option<String>,
    pub tls_cert_days_left: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckStatus {
    Up,
    Down,
    Error,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl From<String> for CheckStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "up" => Self::Up,
            "down" => Self::Down,
            _ => Self::Error,
        }
    }
}

// ───── Notifier ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notifier {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub notifier_type: String,
    pub config_json: serde_json::Value,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ───── Timeline / Dashboard ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatus {
    pub total_monitors: u64,
    pub enabled_monitors: u64,
    pub up_monitors: u64,
    pub down_monitors: u64,
    pub total_checks_24h: u64,
    pub avg_response_time_24h: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSummary {
    pub id: String,
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    pub enabled: bool,
    pub last_status: Option<String>,
    pub last_response_time_ms: Option<u64>,
    pub last_checked_at: Option<String>,
    pub uptime_7d: Option<f64>,
    pub uptime_30d: Option<f64>,
    pub token: Option<String>,
    pub grace_seconds: Option<i64>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub checked_at: String,
    pub status: String,
    pub response_time_ms: Option<u64>,
}

// ───── Timeline Bucket (aggregated) ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBucket {
    pub bucket_start: String, // ISO timestamp of bucket start
    pub up_pct: f64,          // % of checks that were up (0.0 - 100.0)
    pub avg_response_time_ms: f64,
    pub count: i64,
    pub dominant_status: String, // "up", "down", or "error"
}

// ───── Status Page ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusPage {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub monitors: Vec<String>,
    pub public: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StatusPageRow {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub monitors: String,
    pub public: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<StatusPageRow> for StatusPage {
    fn from(row: StatusPageRow) -> Self {
        StatusPage {
            id: row.id,
            slug: row.slug,
            title: row.title,
            description: row.description,
            monitors: serde_json::from_str(&row.monitors).unwrap_or_default(),
            public: row.public != 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// ───── DB row types (SQLx FromRow) ─────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MonitorRow {
    pub id: String,
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    pub config_json: String,
    pub interval_seconds: i64,
    pub timeout_seconds: i64,
    pub enabled: i32,
    pub notifier_id: Option<String>,
    pub confirmations_required: i64,
    pub failed_checks: i64,
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
    pub message_template_expiry: Option<String>,
    pub tags: String,
    pub token: Option<String>,
    pub grace_seconds: Option<i64>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Like MonitorRow but includes summary fields from a LEFT JOIN with checks
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MonitorWithSummaryRow {
    pub id: String,
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    pub config_json: String,
    pub interval_seconds: i64,
    pub timeout_seconds: i64,
    pub enabled: i32,
    pub notifier_id: Option<String>,
    pub confirmations_required: i64,
    pub failed_checks: i64,
    pub latency_threshold_ms: Option<i64>,
    pub message_template_down: Option<String>,
    pub message_template_latency: Option<String>,
    pub message_template_up: Option<String>,
    pub message_template_expiry: Option<String>,
    pub tags: String,
    pub token: Option<String>,
    pub grace_seconds: Option<i64>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    // Summary fields from LEFT JOIN
    pub last_status: Option<String>,
    pub last_response_time_ms: Option<i64>,
    pub last_checked_at: Option<String>,
}

impl From<MonitorWithSummaryRow> for Monitor {
    fn from(row: MonitorWithSummaryRow) -> Self {
        Monitor {
            id: row.id,
            name: row.name,
            monitor_type: row.monitor_type,
            target: row.target,
            config_json: serde_json::from_str(&row.config_json).unwrap_or_default(),
            interval_seconds: row.interval_seconds,
            timeout_seconds: row.timeout_seconds,
            enabled: row.enabled != 0,
            notifier_id: row.notifier_id,
            confirmations_required: row.confirmations_required,
            failed_checks: row.failed_checks,
            latency_threshold_ms: row.latency_threshold_ms,
            message_template_down: row.message_template_down,
            message_template_latency: row.message_template_latency,
            message_template_up: row.message_template_up,
            message_template_expiry: row.message_template_expiry,
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            token: row.token,
            grace_seconds: row.grace_seconds,
            last_seen_at: row.last_seen_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<MonitorRow> for Monitor {
    fn from(row: MonitorRow) -> Self {
        Monitor {
            id: row.id,
            name: row.name,
            monitor_type: row.monitor_type,
            target: row.target,
            config_json: serde_json::from_str(&row.config_json).unwrap_or_default(),
            interval_seconds: row.interval_seconds,
            timeout_seconds: row.timeout_seconds,
            enabled: row.enabled != 0,
            notifier_id: row.notifier_id,
            confirmations_required: row.confirmations_required,
            failed_checks: row.failed_checks,
            latency_threshold_ms: row.latency_threshold_ms,
            message_template_down: row.message_template_down,
            message_template_latency: row.message_template_latency,
            message_template_up: row.message_template_up,
            message_template_expiry: row.message_template_expiry,
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            token: row.token,
            grace_seconds: row.grace_seconds,
            last_seen_at: row.last_seen_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotifierRow {
    pub id: String,
    pub name: String,
    pub notifier_type: String,
    pub config_json: String,
    pub enabled: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<NotifierRow> for Notifier {
    fn from(row: NotifierRow) -> Self {
        Notifier {
            id: row.id,
            name: row.name,
            notifier_type: row.notifier_type,
            config_json: serde_json::from_str(&row.config_json).unwrap_or_default(),
            enabled: row.enabled != 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// ───── Check result (already has FromRow) ─────
// CheckResult already derives sqlx::FromRow above
// The response_time_ms is i64 in DB but we want u64 in API
// SQLx handles this via a separate query type

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TimelinePointRow {
    pub checked_at: String,
    pub status: String,
    pub response_time_ms: i64,
}

impl From<TimelinePointRow> for TimelinePoint {
    fn from(row: TimelinePointRow) -> Self {
        TimelinePoint {
            checked_at: row.checked_at,
            status: row.status,
            response_time_ms: Some(row.response_time_ms as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ───── CheckStatus tests ─────

    #[test]
    fn test_check_status_display_up() {
        assert_eq!(CheckStatus::Up.to_string(), "up");
    }

    #[test]
    fn test_check_status_display_down() {
        assert_eq!(CheckStatus::Down.to_string(), "down");
    }

    #[test]
    fn test_check_status_display_error() {
        assert_eq!(CheckStatus::Error.to_string(), "error");
    }

    #[test]
    fn test_check_status_from_up() {
        let status: CheckStatus = "up".to_string().into();
        assert!(matches!(status, CheckStatus::Up));
    }

    #[test]
    fn test_check_status_from_down() {
        let status: CheckStatus = "down".to_string().into();
        assert!(matches!(status, CheckStatus::Down));
    }

    #[test]
    fn test_check_status_from_error() {
        let status: CheckStatus = "error".to_string().into();
        assert!(matches!(status, CheckStatus::Error));
    }

    #[test]
    fn test_check_status_from_unknown_defaults_to_error() {
        let status: CheckStatus = "unknown".to_string().into();
        assert!(matches!(status, CheckStatus::Error));
    }

    // ───── Monitor ← MonitorRow conversion ─────

    #[test]
    fn test_monitor_from_row() {
        let row = MonitorRow {
            id: "test-id".into(),
            name: "Test Monitor".into(),
            monitor_type: "http".into(),
            target: "https://example.com".into(),
            config_json: r#"{"method":"GET"}"#.into(),
            interval_seconds: 300,
            timeout_seconds: 30,
            enabled: 1,
            notifier_id: Some("notif-1".into()),
            confirmations_required: 2,
            failed_checks: 1,
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
            message_template_expiry: None,
            tags: "[\"web\",\"api\"]".into(),
            created_at: "2026-07-03T08:00:00+00:00".into(),
            updated_at: "2026-07-03T08:00:00+00:00".into(),
        };

        let monitor: Monitor = row.into();

        assert_eq!(monitor.id, "test-id");
        assert_eq!(monitor.name, "Test Monitor");
        assert_eq!(monitor.monitor_type, "http");
        assert_eq!(monitor.target, "https://example.com");
        assert_eq!(monitor.config_json["method"], "GET");
        assert_eq!(monitor.interval_seconds, 300);
        assert_eq!(monitor.timeout_seconds, 30);
        assert!(monitor.enabled);
        assert_eq!(monitor.notifier_id, Some("notif-1".into()));
        assert_eq!(monitor.tags, vec!["web".to_string(), "api".to_string()]);
    }

    #[test]
    fn test_monitor_from_row_disabled() {
        let row = MonitorRow {
            id: "test-id".into(),
            name: "Disabled Monitor".into(),
            monitor_type: "tcp".into(),
            target: "localhost:8080".into(),
            config_json: "{}".into(),
            interval_seconds: 600,
            timeout_seconds: 10,
            enabled: 0,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
            message_template_expiry: None,
            tags: "[]".into(),
            created_at: "2026-07-03T08:00:00+00:00".into(),
            updated_at: "2026-07-03T08:00:00+00:00".into(),
        };

        let monitor: Monitor = row.into();
        assert!(!monitor.enabled);
        assert!(monitor.notifier_id.is_none());
    }

    #[test]
    fn test_monitor_from_row_invalid_config_json() {
        let row = MonitorRow {
            id: "test-id".into(),
            name: "Bad Config".into(),
            monitor_type: "ping".into(),
            target: "8.8.8.8".into(),
            config_json: "not-json".into(),
            interval_seconds: 300,
            timeout_seconds: 5,
            enabled: 1,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
            message_template_expiry: None,
            tags: "[]".into(),
            created_at: "2026-07-03T08:00:00+00:00".into(),
            updated_at: "2026-07-03T08:00:00+00:00".into(),
        };

        let monitor: Monitor = row.into();
        // Falls back to Null
        assert_eq!(monitor.config_json, serde_json::Value::Null);
    }

    // ───── Notifier ← NotifierRow conversion ─────

    #[test]
    fn test_notifier_from_row() {
        let row = NotifierRow {
            id: "notif-1".into(),
            name: "Telegram Alert".into(),
            notifier_type: "telegram".into(),
            config_json: r#"{"bot_token":"abc","chat_id":"-123"}"#.into(),
            enabled: 1,
            created_at: "2026-07-03T08:00:00+00:00".into(),
            updated_at: "2026-07-03T08:00:00+00:00".into(),
        };

        let notifier: Notifier = row.into();

        assert_eq!(notifier.id, "notif-1");
        assert_eq!(notifier.name, "Telegram Alert");
        assert_eq!(notifier.notifier_type, "telegram");
        assert_eq!(notifier.config_json["bot_token"], "abc");
        assert_eq!(notifier.config_json["chat_id"], "-123");
        assert!(notifier.enabled);
    }

    #[test]
    fn test_notifier_from_row_disabled() {
        let row = NotifierRow {
            id: "notif-2".into(),
            name: "Disabled Notifier".into(),
            notifier_type: "telegram".into(),
            config_json: "{}".into(),
            enabled: 0,
            created_at: "2026-07-03T08:00:00+00:00".into(),
            updated_at: "2026-07-03T08:00:00+00:00".into(),
        };

        let notifier: Notifier = row.into();
        assert!(!notifier.enabled);
    }

    // ───── TimelinePoint ← TimelinePointRow conversion ─────

    #[test]
    fn test_timeline_point_from_row() {
        let row = TimelinePointRow {
            checked_at: "2026-07-03T08:00:00+00:00".into(),
            status: "up".into(),
            response_time_ms: 42,
        };

        let point: TimelinePoint = row.into();

        assert_eq!(point.checked_at, "2026-07-03T08:00:00+00:00");
        assert_eq!(point.status, "up");
        assert_eq!(point.response_time_ms, Some(42));
    }

    #[test]
    fn test_timeline_point_from_row_zero_ms() {
        let row = TimelinePointRow {
            checked_at: "2026-07-03T08:00:00+00:00".into(),
            status: "down".into(),
            response_time_ms: 0,
        };

        let point: TimelinePoint = row.into();
        assert_eq!(point.response_time_ms, Some(0));
    }

    // ───── Monitor serialization ─────

    #[test]
    fn test_monitor_serde_rename_type() {
        let monitor = Monitor {
            id: "m-1".into(),
            name: "Test".into(),
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
            created_at: "now".into(),
            updated_at: "now".into(),
        };

        let json = serde_json::to_value(&monitor).unwrap();
        assert_eq!(json["type"], "http");
        assert!(json.get("monitor_type").is_none());
    }

    #[test]
    fn test_notifier_serde_rename_type() {
        let notifier = Notifier {
            id: "n-1".into(),
            name: "Telegram".into(),
            notifier_type: "telegram".into(),
            config_json: serde_json::json!({}),
            enabled: true,
            created_at: "now".into(),
            updated_at: "now".into(),
        };

        let json = serde_json::to_value(&notifier).unwrap();
        assert_eq!(json["type"], "telegram");
        assert!(json.get("notifier_type").is_none());
    }
}
