use reqwest::header::HeaderMap;
use reqwest::Client;

use crate::models::{CheckResult, Monitor};

pub async fn send_ntfy_notification(
    topic: &str,
    server_url: &str,
    token: Option<&str>,
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

    let title = format!("{} {} — {}", emoji, direction, monitor.name)
        .trim()
        .to_string();
    let message = format!(
        "Target: {}\nStatus: {}\nResponse: {}ms\n{}",
        monitor.target,
        check.status,
        check.response_time_ms,
        check.error_message.as_deref().unwrap_or("")
    );

    let url = format!("{}/{}", server_url.trim_end_matches('/'), topic);

    let mut headers = HeaderMap::new();
    headers.insert("Title", title.parse().unwrap());
    headers.insert("Tags", emoji.parse().unwrap());
    if let Some(t) = token {
        let auth_val = format!("Bearer {}", t);
        headers.insert("Authorization", auth_val.parse().unwrap());
    }

    let client = Client::new();
    let resp = client
        .post(&url)
        .headers(headers)
        .body(message)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("ntfy error: {} — {}", status, body);
    }

    Ok(())
}
