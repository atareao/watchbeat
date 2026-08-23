use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_gotify_notification(
    server_url: &str,
    app_token: &str,
    priority: i64,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let emoji = match check.status.as_str() {
        "up" => "\u{1f7e2}",
        _ => "\u{1f534}",
    };

    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let title = format!("{} {} — {}", emoji, direction, monitor.name).trim().to_string();
    let message = format!(
        "Target: {}\nStatus: {}\nResponse: {}ms\n{}",
        monitor.target,
        check.status,
        check.response_time_ms,
        check.error_message.as_deref().unwrap_or("")
    );

    let url = format!("{}/message?token={}", server_url.trim_end_matches('/'), app_token);

    let payload = json!({
        "title": title,
        "message": message,
        "priority": priority,
    });

    let client = Client::new();
    let resp = client.post(&url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Gotify error: {} — {}", status, body);
    }

    Ok(())
}