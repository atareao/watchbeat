use reqwest::Client;

use crate::models::{CheckResult, Monitor};

/// Send a notification via Telegram. Called from the scheduler after resolving
/// notifier config from DB.
pub async fn send_telegram_notification(
    bot_token: &str,
    chat_id: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let emoji = match check.status.as_str() {
        "up" => "\u{1f7e2}",
        _ => "\u{1f534}",
    };

    let direction = if was_up && check.status != "up" {
        "CA\u{cd}DO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let text = format!("{} {} — {}", emoji, direction, monitor.name);
    let details = format!(
        "Target: {}\nStatus: {}\nResponse: {}ms\n{}",
        monitor.target,
        check.status,
        check.response_time_ms,
        check.error_message.as_deref().unwrap_or("")
    );

    let message = format!("*{}*\n\n{}", text, details);
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

    let client = Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown",
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API error: {} — {}", status, body);
    }

    Ok(())
}
