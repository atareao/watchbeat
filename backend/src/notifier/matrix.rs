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
        emoji, direction, monitor.name, monitor.target, check.status, check.response_time_ms,
    );

    let details = check.error_message.as_deref().unwrap_or("");

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

    /// Read a complete HTTP request (headers + body) from a TCP stream.
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
            // Check if we have the complete headers (end with \r\n\r\n)
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
    async fn test_matrix_sends_correct_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Verify method and path
            assert!(
                request.contains("PUT"),
                "Expected PUT method, got:\n{}",
                request
            );
            assert!(
                request.contains("/_matrix/client/v3/rooms/!room:id/send/m.room.message/"),
                "Missing Matrix room path:\n{}",
                request
            );

            // Verify auth header (reqwest lowercases header names)
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer test_token"),
                "Missing Authorization header:\n{}",
                request
            );

            // Verify JSON body fields
            assert!(request.contains("msgtype"));
            assert!(request.contains("m.text"));
            assert!(request.contains("Test Monitor"));
            assert!(request.contains("https://example.com"));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_matrix_notification(
            &server_url,
            "test_token",
            "!room:id",
            &monitor,
            &check,
            false,
        )
        .await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_matrix_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        // Connect to a closed port
        let result = send_matrix_notification(
            "http://127.0.0.1:1",
            "token",
            "!room:id",
            &monitor,
            &check,
            true,
        )
        .await;
        assert!(
            result.is_err(),
            "Expected connection error, got Ok: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_matrix_direction_caido() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Direction "CAÍDO" should appear in the body when was_up=true and status=down
            assert!(
                request.contains("CAÍDO"),
                "Expected CAÍDO in body:\n{}",
                request
            );
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result = send_matrix_notification(
            &server_url,
            "test_token",
            "!room:id",
            &monitor,
            &check,
            true,
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_matrix_direction_recuperado() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Direction "RECUPERADO" should appear when was_up=false and status=up
            assert!(
                request.contains("RECUPERADO"),
                "Expected RECUPERADO in body:\n{}",
                request
            );
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_matrix_notification(
            &server_url,
            "test_token",
            "!room:id",
            &monitor,
            &check,
            false,
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_matrix_direction_same() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);
            // Same status (was_up=true, status=up) → direction is empty
            // The body should contain "🟢  — Test Monitor" (empty direction)
            assert!(
                request.contains("🟢"),
                "Expected green emoji for up status:\n{}",
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

        let result = send_matrix_notification(
            &server_url,
            "test_token",
            "!room:id",
            &monitor,
            &check,
            true,
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }
}
