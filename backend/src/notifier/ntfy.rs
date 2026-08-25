use reqwest::header::HeaderMap;
use reqwest::Client;

use crate::models::{CheckResult, Monitor};

pub async fn send_ntfy_notification(
    topic: &str,
    server_url: &str,
    token: Option<&str>,
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

    let url = format!("{}/{}", server_url.trim_end_matches('/'), topic);

    let mut headers = HeaderMap::new();
    headers.insert("Title", title.parse().unwrap());
    headers.insert("Tags", emoji.parse().unwrap());
    if let Some(t) = token {
        let auth_val = format!("Bearer {}", t);
        headers.insert("Authorization", auth_val.parse().unwrap());
    }

    let client = Client::new();
    let resp = client
        .post(&url)
        .headers(headers)
        .body(message)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("ntfy error: {} — {}", status, body);
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
    async fn test_ntfy_sends_correct_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Verify POST method and correct path (/mytopic)
            assert!(request.contains("POST"));
            assert!(request.contains("/mytopic"));

            // Verify custom headers (reqwest lowercases them)
            assert!(
                request.to_lowercase().contains("title:"),
                "Missing Title header:\n{}",
                request
            );
            assert!(
                request.to_lowercase().contains("tags:"),
                "Missing Tags header:\n{}",
                request
            );

            // Verify body is plain text (not JSON)
            assert!(request.contains("Target: https://example.com"));
            assert!(request.contains("Status: up"));
            assert!(request.contains("Response: 42ms"));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result =
            send_ntfy_notification("mytopic", &server_url, None, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_with_bearer_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Verify Authorization header is present with Bearer token (lowercased by reqwest)
            assert!(
                request.to_lowercase().contains("authorization: bearer my_secret_token"),
                "Missing Bearer auth:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("error"));

        let result = send_ntfy_notification(
            "mytopic",
            &server_url,
            Some("my_secret_token"),
            &monitor,
            &check,
            true,
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_without_token_no_auth_header() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // When no token is provided, there should be NO Authorization header
            assert!(
                !request.contains("Authorization:"),
                "Should NOT have Authorization header without token:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 100, None);

        let result =
            send_ntfy_notification("mytopic", &server_url, None, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result =
            send_ntfy_notification("topic", "http://127.0.0.1:1", None, &monitor, &check, true)
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ntfy_direction_caido() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Direction should be visible in the Title header
            assert!(
                request.contains("CAÍDO"),
                "Expected CAÍDO in Title header:\n{}",
                request
            );
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result =
            send_ntfy_notification("topic", &server_url, None, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_direction_recuperado() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            assert!(
                request.contains("RECUPERADO"),
                "Expected RECUPERADO in Title header:\n{}",
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
            send_ntfy_notification("topic", &server_url, None, &monitor, &check, false).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_direction_same() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Same status: direction is empty, title is just "🟢 — Test Monitor"
            // Check that Title header (lowercased by reqwest) starts with the emoji
            assert!(
                request.contains("\u{1f7e2}"),
                "Expected green emoji in title:\n{}",
                request
            );
            assert!(
                !request.contains("CAÍDO") && !request.contains("RECUPERADO"),
                "Direction should be empty when no transition:\n{}",
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
            send_ntfy_notification("topic", &server_url, None, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }
}
