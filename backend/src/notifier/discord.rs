use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_discord_notification(
    webhook_url: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let color = match check.status.as_str() {
        "up" => 5763719,
        _ => 15548997,
    };

    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let title = if direction.is_empty() {
        format!("Check: {}", monitor.name)
    } else {
        format!("{} — {}", direction, monitor.name)
    };

    let payload = json!({
        "embeds": [{
            "title": title,
            "color": color,
            "fields": [
                { "name": "Target", "value": monitor.target, "inline": true },
                { "name": "Status", "value": check.status, "inline": true },
                { "name": "Response", "value": format!("{}ms", check.response_time_ms), "inline": true },
                { "name": "Error", "value": check.error_message.as_deref().unwrap_or("—"), "inline": false },
            ],
            "footer": { "text": "Vigilatrs" },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }]
    });

    let client = Client::new();
    let resp = client.post(webhook_url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Discord error: {} — {}", status, body);
    }

    Ok(())
}