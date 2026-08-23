use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MonitorConfig {
    Http {
        method: Option<String>,
        expected_status: Option<u16>,
        headers: Option<serde_json::Value>,
        body: Option<String>,
    },
    Tcp {},
    Ping {},
}

// ───── Check Result ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: i64,
    pub monitor_id: String,
    pub status: String, // "up" | "down" | "error"
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub checked_at: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub checked_at: String,
    pub status: String,
    pub response_time_ms: Option<u64>,
}