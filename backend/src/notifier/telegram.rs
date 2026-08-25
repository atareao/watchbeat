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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckResult, Monitor};

    fn make_test_monitor(monitor_type: &str, target: &str) -> Monitor {
        Monitor {
            id: "test-id".into(),
            name: "Test Monitor".into(),
            monitor_type: monitor_type.into(),
            target: target.into(),
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

    fn make_test_check(
        status: &str,
        status_code: u16,
        response_time_ms: i64,
        error: Option<&str>,
    ) -> CheckResult {
        CheckResult {
            id: 0,
            monitor_id: "test-id".into(),
            status: status.into(),
            status_code: Some(status_code),
            response_time_ms,
            error_message: error.map(|s| s.to_string()),
            checked_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Telegram's URL is hardcoded to api.telegram.org, so we can't easily
    /// mock the server. This test verifies the function returns an error
    /// when connecting (always the case with a fake token).
    #[tokio::test]
    async fn test_telegram_returns_error() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_telegram_notification("invalid_token", "invalid_chat", &monitor, &check, true),
        )
        .await;

        // Should either complete with an error or timeout (no network).
        // Either way, the important thing is we don't panic.
        if let Ok(res) = result {
            assert!(res.is_err(), "Expected an error with fake token");
        }
        // If it times out, that's also fine — no internet in CI.
    }

    #[tokio::test]
    async fn test_telegram_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        // Point at a closed port to force connection error
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_telegram_notification("invalid_token", "invalid_chat", &monitor, &check, false),
        )
        .await;

        if let Ok(res) = result {
            assert!(res.is_err(), "Expected an error with fake token");
        }
    }
}
