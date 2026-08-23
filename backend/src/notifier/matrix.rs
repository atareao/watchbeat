use crate::models::{CheckResult, Monitor};

/// Send a notification via Matrix. Called from the scheduler after resolving
/// notifier config from DB.
pub async fn send_matrix_notification(
    homeserver_url: &str,
    access_token: &str,
    room_id: &str,
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
        "{} {} — {}\nTarget: {}\nStatus: {}\nResponse: {}ms",
        emoji,
        direction,
        monitor.name,
        monitor.target,
        check.status,
        check.response_time_ms,
    );

    let details = check
        .error_message
        .as_deref()
        .unwrap_or("");

    let message = if details.is_empty() {
        text
    } else {
        format!("{}\n{}", text, details)
    };

    let txn_id = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
        homeserver_url.trim_end_matches('/'),
        room_id,
        txn_id
    );

    let payload = serde_json::json!({
        "msgtype": "m.text",
        "body": message,
    });

    let client = reqwest::Client::new();
    let resp = client
        .put(&url)
        .bearer_auth(access_token)
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Matrix API error: {} — {}", status, body);
    }

    Ok(())
}