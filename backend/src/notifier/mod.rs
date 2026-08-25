use async_trait::async_trait;

use crate::models::{CheckResult, Monitor, Notifier};

pub mod discord;
pub mod email;
pub mod gotify;
pub mod matrix;
pub mod ntfy;
pub mod slack;
pub mod telegram;
pub mod webhook;

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
        "telegram" => Some(Box::new(TelegramNotifier::new(notifier.config_json.clone()))),
        "matrix" => Some(Box::new(MatrixNotifier::new(notifier.config_json.clone()))),
        "ntfy" => Some(Box::new(NtfyNotifier::new(notifier.config_json.clone()))),
        "webhook" => Some(Box::new(WebhookNotifier::new(notifier.config_json.clone()))),
        "slack" => Some(Box::new(SlackNotifier::new(notifier.config_json.clone()))),
        "discord" => Some(Box::new(DiscordNotifier::new(notifier.config_json.clone()))),
        "email" => Some(Box::new(EmailNotifier::new(notifier.config_json.clone()))),
        "gotify" => Some(Box::new(GotifyNotifier::new(notifier.config_json.clone()))),
        _ => None,
    }
}

// ───── Telegram Notifier ─────

pub struct TelegramNotifier {
    config: serde_json::Value,
}

impl TelegramNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for TelegramNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let bot_token = self
            .config
            .get("bot_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing bot_token in telegram notifier config"))?;
        let chat_id = self
            .config
            .get("chat_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing chat_id in telegram notifier config"))?;
        crate::notifier::telegram::send_telegram_notification(bot_token, chat_id, monitor, check, was_up).await
    }
}

// ───── Matrix Notifier ─────

pub struct MatrixNotifier {
    config: serde_json::Value,
}

impl MatrixNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for MatrixNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let homeserver_url = self
            .config
            .get("homeserver_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing homeserver_url in matrix notifier config"))?;
        let access_token = self
            .config
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing access_token in matrix notifier config"))?;
        let room_id = self
            .config
            .get("room_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing room_id in matrix notifier config"))?;
        crate::notifier::matrix::send_matrix_notification(
            homeserver_url,
            access_token,
            room_id,
            monitor,
            check,
            was_up,
        )
        .await
    }
}

// ───── Ntfy Notifier ─────

pub struct NtfyNotifier {
    config: serde_json::Value,
}

impl NtfyNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for NtfyNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let topic = self
            .config
            .get("topic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing topic in ntfy notifier config"))?;
        let server_url = self
            .config
            .get("server_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://ntfy.sh");
        let token = self.config.get("token").and_then(|v| v.as_str());
        crate::notifier::ntfy::send_ntfy_notification(topic, server_url, token, monitor, check, was_up).await
    }
}

// ───── Webhook Notifier ─────

pub struct WebhookNotifier {
    config: serde_json::Value,
}

impl WebhookNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for WebhookNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let url = self
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing url in webhook notifier config"))?;
        let method = self
            .config
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("POST");
        let headers_json = self
            .config
            .get("headers")
            .map(|v| v.to_string())
            .unwrap_or_default();
        crate::notifier::webhook::send_webhook_notification(url, method, &headers_json, monitor, check, was_up).await
    }
}

// ───── Slack Notifier ─────

pub struct SlackNotifier {
    config: serde_json::Value,
}

impl SlackNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for SlackNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let webhook_url = self
            .config
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing webhook_url in slack notifier config"))?;
        crate::notifier::slack::send_slack_notification(webhook_url, monitor, check, was_up).await
    }
}

// ───── Discord Notifier ─────

pub struct DiscordNotifier {
    config: serde_json::Value,
}

impl DiscordNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for DiscordNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let webhook_url = self
            .config
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing webhook_url in discord notifier config"))?;
        crate::notifier::discord::send_discord_notification(webhook_url, monitor, check, was_up).await
    }
}

// ───── Email Notifier ─────

pub struct EmailNotifier {
    config: serde_json::Value,
}

impl EmailNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for EmailNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let smtp_host = self
            .config
            .get("smtp_host")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing smtp_host in email notifier config"))?;
        let smtp_port = self
            .config
            .get("smtp_port")
            .and_then(|v| v.as_u64())
            .unwrap_or(587) as u16;
        let username = self
            .config
            .get("username")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing username in email notifier config"))?;
        let password = self
            .config
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing password in email notifier config"))?;
        let from = self
            .config
            .get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing from in email notifier config"))?;
        let to = self
            .config
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing to in email notifier config"))?;
        crate::notifier::email::send_email_notification(
            smtp_host,
            smtp_port,
            username,
            password,
            from,
            to,
            monitor,
            check,
            was_up,
        )
        .await
    }
}

// ───── Gotify Notifier ─────

pub struct GotifyNotifier {
    config: serde_json::Value,
}

impl GotifyNotifier {
    pub fn new(config: serde_json::Value) -> Self {
        Self { config }
    }
}

#[async_trait]
impl NotifierTrait for GotifyNotifier {
    async fn notify(
        &self,
        monitor: &Monitor,
        check: &CheckResult,
        was_up: bool,
    ) -> anyhow::Result<()> {
        let server_url = self
            .config
            .get("server_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8080");
        let app_token = self
            .config
            .get("app_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing app_token in gotify notifier config"))?;
        let priority = self
            .config
            .get("priority")
            .and_then(|v| v.as_i64())
            .unwrap_or(5);
        crate::notifier::gotify::send_gotify_notification(server_url, app_token, priority, monitor, check, was_up).await
    }
}