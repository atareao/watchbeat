use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_slack_notification(
    webhook_url: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let color = match check.status.as_str() {
        "up" => "#22c55e",
        _ => "#ef4444",
    };

    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let payload = json!({
        "attachments": [{
            "color": color,
            "title": format!("{} — {}", direction, monitor.name),
            "fields": [
                { "title": "Target", "value": monitor.target, "short": true },
                { "title": "Status", "value": check.status, "short": true },
                { "title": "Response", "value": format!("{}ms", check.response_time_ms), "short": true },
                { "title": "Error", "value": check.error_message.as_deref().unwrap_or("—"), "short": false },
            ],
            "footer": "Vigilatrs",
            "ts": chrono::Utc::now().timestamp(),
        }]
    });

    let client = Client::new();
    let resp = client.post(webhook_url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Slack error: {} — {}", status, body);
    }

    Ok(())
}
