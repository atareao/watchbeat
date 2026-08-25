use reqwest::Client;
use serde_json::json;

use crate::models::{CheckResult, Monitor};

pub async fn send_webhook_notification(
    url: &str,
    method: &str,
    headers_json: &str,
    monitor: &Monitor,
    check: &CheckResult,
    was_up: bool,
) -> anyhow::Result<()> {
    let direction = if was_up && check.status != "up" {
        "CAÍDO"
    } else if !was_up && check.status == "up" {
        "RECUPERADO"
    } else {
        ""
    };

    let payload = json!({
        "monitor": monitor.name,
        "monitor_id": monitor.id,
        "target": monitor.target,
        "status": check.status,
        "response_time_ms": check.response_time_ms,
        "error_message": check.error_message,
        "checked_at": check.checked_at,
        "direction": direction,
        "event": if !direction.is_empty() { "change" } else { "check" },
    });

    let client = Client::new();
    let req = match method.to_uppercase().as_str() {
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        _ => client.post(url),
    };

    let mut req = req.json(&payload);

    if !headers_json.is_empty() {
        if let Ok(extra) = serde_json::from_str::<serde_json::Value>(headers_json) {
            if let Some(obj) = extra.as_object() {
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        req = req.header(k.as_str(), val);
                    }
                }
            }
        }
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Webhook error: {} — {}", status, body);
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
    async fn test_webhook_sends_post_by_default() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/hook", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Default is POST
            assert!(
                request.starts_with("POST"),
                "Expected POST, got:\n{}",
                request
            );
            assert!(request.contains("/hook"));

            // Verify JSON body fields
            assert!(request.contains("\"monitor\":\"Test Monitor\""));
            assert!(request.contains("\"status\":\"up\""));
            assert!(request.contains("\"event\":\"check\""));
            assert!(request.contains("\"response_time_ms\":42"));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_webhook_notification(&url, "", "{}", &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_sends_put_when_method_is_put() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/hook", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Should be PUT when method="PUT"
            assert!(
                request.starts_with("PUT"),
                "Expected PUT, got:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_webhook_notification(&url, "PUT", "{}", &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_sends_patch_when_method_is_patch() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/hook", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            assert!(
                request.starts_with("PATCH"),
                "Expected PATCH, got:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let result = send_webhook_notification(&url, "PATCH", "{}", &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_with_custom_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/hook", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Custom headers from JSON string should appear (reqwest lowercases them)
            let lower = request.to_lowercase();
            assert!(
                lower.contains("x-custom: myvalue"),
                "Missing X-Custom header:\n{}",
                request
            );
            assert!(
                lower.contains("authorization: bearer abc123"),
                "Missing Authorization header:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("up", 200, 42, None);

        let headers = r#"{"X-Custom":"myvalue","Authorization":"Bearer abc123"}"#;
        let result =
            send_webhook_notification(&url, "POST", headers, &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_event_change_on_transition() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/hook", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Transition (was_up=true, status=down) → event="change", direction="CAÍDO"
            assert!(
                request.contains("\"event\":\"change\""),
                "Expected change event:\n{}",
                request
            );
            assert!(
                request.contains("\"direction\":\"CAÍDO\""),
                "Expected CAÍDO direction:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result = send_webhook_notification(&url, "POST", "{}", &monitor, &check, true).await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_connection_refused() {
        let monitor = make_test_monitor("http", "https://example.com");
        let check = make_test_check("down", 500, 100, Some("timeout"));

        let result =
            send_webhook_notification("http://127.0.0.1:1", "POST", "{}", &monitor, &check, true)
                .await;
        assert!(result.is_err());
    }
}
