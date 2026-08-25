use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_gotify_notification(
    server_url: &str,
    app_token: &str,
    priority: i64,
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

    let title = format!("{} {} — {}", emoji, direction, monitor.name)
        .trim()
        .to_string();
    let message = format!(
        "Target: {}\nStatus: {}\nResponse: {}ms\n{}",
        monitor.target,
        check.status,
        check.response_time_ms,
        check.error_message.as_deref().unwrap_or("")
    );

    let url = format!(
        "{}/message?token={}",
        server_url.trim_end_matches('/'),
        app_token
    );

    let payload = json!({
        "title": title,
        "message": message,
        "priority": priority,
    });

    let client = Client::new();
    let resp = client.post(&url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Gotify error: {} — {}", status, body);
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
    async fn test_gotify_sends_correct_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // POST method with token in query param
            assert!(request.starts_with("POST"));
            assert!(request.contains("/message?token=my_app_token"));

            // JSON body with title, message, priority
            assert!(request.contains("\"title\":"));
            assert!(request.contains("\"message\":"));
            assert!(request.contains("\"priority\":5"));

            // Body should contain monitor info
            assert!(request.contains("https://example.com"));
            assert!(request.contains("42ms"));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result =
            send_gotify_notification(&server_url, "my_app_token", 5, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_custom_priority() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Priority 10 (high urgency)
            assert!(
                request.contains("\"priority\":10"),
                "Expected priority 10:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, None);

        let result =
            send_gotify_notification(&server_url, "token", 10, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_priority_zero() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Priority 0 (low)
            assert!(
                request.contains("\"priority\":0"),
                "Expected priority 0:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result =
            send_gotify_notification(&server_url, "token", 0, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_direction_caido() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

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

        let result =
            send_gotify_notification(&server_url, "token", 5, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_direction_recuperado() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

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

        let result =
            send_gotify_notification(&server_url, "token", 5, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_direction_same() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Same status → direction empty, just emoji + name
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

        let result =
            send_gotify_notification(&server_url, "token", 5, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gotify_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result =
            send_gotify_notification("http://127.0.0.1:1", "token", 5, &monitor, &check, true)
                .await;
        assert!(result.is_err());
    }
}
