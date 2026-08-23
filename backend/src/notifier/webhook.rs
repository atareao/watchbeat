use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_webhook_notification(
    url: &str,
    method: &str,
    headers_json: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let payload = json!({
        "monitor": monitor.name,
        "monitor_id": monitor.id,
        "target": monitor.target,
        "status": check.status,
        "response_time_ms": check.response_time_ms,
        "error_message": check.error_message,
        "checked_at": check.checked_at,
        "direction": direction,
        "event": if !direction.is_empty() { "change" } else { "check" },
    });

    let client = Client::new();
    let req = match method.to_uppercase().as_str() {
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        _ => client.post(url),
    };

    let mut req = req.json(&payload);

    if !headers_json.is_empty() {
        if let Ok(extra) = serde_json::from_str::<serde_json::Value>(headers_json) {
            if let Some(obj) = extra.as_object() {
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        req = req.header(k.as_str(), val);
                    }
                }
            }
        }
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Webhook error: {} — {}", status, body);
    }

    Ok(())
}