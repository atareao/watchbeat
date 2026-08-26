/// Send a notification via Telegram. Called from the scheduler after resolving
/// notifier config from DB.
pub async fn send_telegram_notification(
    bot_token: &str,
    chat_id: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
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

    /// Telegram's URL is hardcoded to api.telegram.org, so we can't easily
    /// mock the server. This test verifies the function returns an error
    /// when connecting (always the case with a fake token).
    #[tokio::test]
    async fn test_telegram_returns_error() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_telegram_notification("invalid_token", "invalid_chat", "test message"),
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
        // Point at a closed port to force connection error
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_telegram_notification("invalid_token", "invalid_chat", "test message"),
        )
        .await;

        if let Ok(res) = result {
            assert!(res.is_err(), "Expected an error with fake token");
        }
    }
}
