use async_trait::async_trait;

use crate::models::{CheckResult, Monitor, Notifier};

#[async_trait]
pub trait NotifierTrait: Send + Sync {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()>;
}

/// Build a notifier for the given notifier config.
pub fn notifier_for(notifier: &Notifier) -> Option<Box<dyn NotifierTrait>> {
    match notifier.notifier_type.as_str() {
        "telegram" => Some(Box::new(TelegramNotifier)),
        _ => None,
    }
}

// ───── Telegram Notifier ─────

pub struct TelegramNotifier;

#[async_trait]
impl NotifierTrait for TelegramNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let bot_token = monitor
            .config_json
            .get("bot_token")
            .and_then(|v| v.as_str())
            .or_else(|| {
                // Also check notifier config if stored there
                None
            });

        // We look for the token in the notifier config, not the monitor
        // Since monitors only reference notifier_id, the token lives in the notifier
        anyhow::bail!("Telegram notifier needs notifier config — use the notifier's config_json")
    }
}

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
        "up" => "🟢",
        _ => "🔴",
    };

    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let text = format!(
        "{} {} — {}",
        emoji, direction, monitor.name
    );

    let details = format!(
        "Target: {}\nStatus: {}\nResponse: {}ms\n{}",
        monitor.target,
        check.status,
        check.response_time_ms,
        check
            .error_message
            .as_deref()
            .unwrap_or("")
    );

    let message = format!("*{}*\n\n{}", text, details);

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        bot_token
    );

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
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Telegram API error: {}", body);
    }

    Ok(())
}