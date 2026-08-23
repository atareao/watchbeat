use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use lettre::message::header::ContentType;

use crate::models::{CheckResult, Monitor};

pub async fn send_email_notification(
    smtp_host: &str,
    smtp_port: u16,
    username: &str,
    password: &str,
    from: &str,
    to: &str,
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

    let subject = format!("{} {} — {}", emoji, direction, monitor.name);
    let body = format!(
        "{}\n\nMonitor: {}\nTarget: {}\nStatus: {}\nResponse: {}ms\nError: {}\nChecked at: {}",
        subject,
        monitor.name,
        monitor.target,
        check.status,
        check.response_time_ms,
        check.error_message.as_deref().unwrap_or("—"),
        check.checked_at,
    );

    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)?;

    let creds = Credentials::new(username.to_string(), password.to_string());

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
            .port(smtp_port)
            .credentials(creds)
            .build();

    mailer.send(email).await?;

    Ok(())
}