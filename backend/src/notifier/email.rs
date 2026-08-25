use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::models::{CheckResult, Monitor};

#[allow(clippy::too_many_arguments)]
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

    /// SMTP uses lettre's transport which can't be easily mocked via TCP.
    /// Test that connecting to a closed port returns a connection error.
    #[tokio::test]
    async fn test_email_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        // Point to a closed port — will get connection refused
        // Setting a short timeout so the test doesn't hang
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_email_notification(
                "127.0.0.1",
                1,
                "user",
                "pass",
                "from@example.com",
                "to@example.com",
                &monitor,
                &check,
                true,
            ),
        )
        .await;

        let err = result
            .expect("Email notification timed out")
            .expect_err("Expected error connecting to closed port");

        // The error should relate to connection failure
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

    /// Verify that an invalid SMTP hostname returns an error.
    #[tokio::test]
    async fn test_email_invalid_smtp_host() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        // A host that doesn't exist should produce a DNS/connection error
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            send_email_notification(
                "smtp.doesnotexist.example.invalid",
                587,
                "user",
                "pass",
                "from@example.com",
                "to@example.com",
                &monitor,
                &check,
                false,
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
