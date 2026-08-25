use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_discord_notification(
    webhook_url: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let color = match check.status.as_str() {
        "up" => 5763719,
        _ => 15548997,
    };

    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let title = if direction.is_empty() {
        format!("Check: {}", monitor.name)
    } else {
        format!("{} — {}", direction, monitor.name)
    };

    let payload = json!({
        "embeds": [{
            "title": title,
            "color": color,
            "fields": [
                { "name": "Target", "value": monitor.target, "inline": true },
                { "name": "Status", "value": check.status, "inline": true },
                { "name": "Response", "value": format!("{}ms", check.response_time_ms), "inline": true },
                { "name": "Error", "value": check.error_message.as_deref().unwrap_or("—"), "inline": false },
            ],
            "footer": { "text": "Vigilatrs" },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }]
    });

    let client = Client::new();
    let resp = client.post(webhook_url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Discord error: {} — {}", status, body);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckResult, Monitor};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut buf = vec![0u8; 16384];
        let mut total = 0usize;
        loop {
            let n = stream.read(&mut buf[total..]).await.unwrap();
            if n == 0 {
                break;
            }
            total += n;
            let data = &buf[..total];
            if let Ok(s) = std::str::from_utf8(data) {
                if let Some(pos) = s.find("\r\n\r\n") {
                    let header_end = pos + 4;
                    let content_length = s[..pos]
                        .lines()
                        .find_map(|line| {
                            if line.to_lowercase().starts_with("content-length:") {
                                line.split(':').nth(1)?.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let total_expected = header_end + content_length;
                    if total >= total_expected {
                        break;
                    }
                }
            }
        }
        buf[..total].to_vec()
    }

    #[tokio::test]
    async fn test_discord_sends_correct_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // POST method
            assert!(request.starts_with("POST"));

            // JSON body with Discord-style embeds
            assert!(request.contains("embeds"));
            assert!(request.contains("\"CAÍDO — Test Monitor\""));
            assert!(request.contains("\"color\":15548997"));
            assert!(request.contains("\"footer\":{\"text\":\"Vigilatrs\""));

            // Fields
            assert!(request.contains("\"Target\""));
            assert!(request.contains("\"Status\""));
            assert!(request.contains("\"Response\""));
            assert!(request.contains("42ms"));
            assert!(request.contains("\"Error\""));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 42, Some("timeout"));

        let result = send_discord_notification(&url, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_up_status_color_green() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Up status → color 5763719 (green)
            assert!(
                request.contains("\"color\":5763719"),
                "Expected green color (5763719) for up status in:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_discord_notification(&url, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_down_status_color_red() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Down status → color 15548997 (red)
            assert!(
                request.contains("\"color\":15548997"),
                "Expected red color (15548997) for down status in:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, None);

        let result = send_discord_notification(&url, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_title_check_when_no_transition() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Same status: title is "Check: Test Monitor" (direction empty)
            assert!(
                request.contains("\"Check: Test Monitor\""),
                "Expected 'Check: Test Monitor' title:\n{}",
                request
            );
            assert!(
                !request.contains("CAÍDO") && !request.contains("RECUPERADO"),
                "Direction should be empty when same status:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_discord_notification(&url, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_direction_caido() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            assert!(
                request.contains("CAÍDO"),
                "Expected CAÍDO in title:\n{}",
                request
            );
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, None);

        let result = send_discord_notification(&url, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_direction_recuperado() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/discord", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            assert!(
                request.contains("RECUPERADO"),
                "Expected RECUPERADO in title:\n{}",
                request
            );
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_discord_notification(&url, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_discord_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result = send_discord_notification("http://127.0.0.1:1", &monitor, &check, true).await;
        assert!(result.is_err());
    }
}
