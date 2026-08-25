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
        _check: &CheckResult,
        _was_up: bool,
    ) -> anyhow::Result<()> {
        let _bot_token = monitor
            .config_json
            .get("bot_token")
            .and_then(|v| v.as_str())
            .or({
                // Also check notifier config if stored there
                None
            });

        // We look for the token in the notifier config, not the monitor
        // Since monitors only reference notifier_id, the token lives in the notifier
        anyhow::bail!("Telegram notifier needs notifier config — use the notifier's config_json")
    }
}
