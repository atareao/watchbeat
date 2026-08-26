use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[allow(clippy::too_many_arguments)]
pub async fn send_email_notification(
    smtp_host: &str,
    smtp_port: u16,
    username: &str,
    password: &str,
    from: &str,
    to: &str,
    message: &str,
) -> anyhow::Result<()> {
    let email = Message::builder()
        .from(from.parse::<Mailbox>().map_err(|e| anyhow::anyhow!("Invalid from address: {}", e))?)
        .to(to.parse::<Mailbox>().map_err(|e| anyhow::anyhow!("Invalid to address: {}", e))?)
        .subject("WatchBeat Alert")
        .body(message.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to build email: {}", e))?;

    let creds = Credentials::new(username.to_string(), password.to_string());
    let transport = SmtpTransport::starttls_relay(smtp_host)
        .map_err(|e| anyhow::anyhow!("SMTP relay error: {}", e))?
        .port(smtp_port)
        .credentials(creds)
        .build();

    transport
        .send(&email)
        .map_err(|e| anyhow::anyhow!("Failed to send email: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_email_connection_refused() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_email_notification(
                "127.0.0.1",
                1,
                "user",
                "pass",
                "from@example.com",
                "to@example.com",
                "test message",
            ),
        )
        .await;

        let err = result
            .expect("Email notification timed out")
            .expect_err("Expected error connecting to closed port");

        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("connection")
                || err_str.contains("refused")
                || err_str.contains("timed out")
                || err_str.contains("eof")
                || err_str.contains("tls"),
            "Expected connection-related error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_email_invalid_smtp_host() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_email_notification(
                "smtp.doesnotexist.example.invalid",
                587,
                "user",
                "pass",
                "from@example.com",
                "to@example.com",
                "test message",
            ),
        )
        .await;

        let err = result
            .expect("Email notification timed out")
            .expect_err("Expected error with invalid SMTP host");

        let err_str = err.to_string().to_lowercase();
        assert!(
            err_str.contains("dns")
                || err_str.contains("resolve")
                || err_str.contains("connection")
                || err_str.contains("refused"),
            "Expected DNS/connection error, got: {}",
            err
        );
    }
}