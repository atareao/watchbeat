pub async fn send_webhook_notification(
    url: &str,
    method: &str,
    headers_json: &str,
    message: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut req = match method.to_uppercase().as_str() {
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        _ => client.post(url),
    };

    if !headers_json.is_empty() {
        if let Ok(headers) =
            serde_json::from_str::<std::collections::HashMap<String, String>>(headers_json)
        {
            for (k, v) in headers {
                req = req.header(&k, &v);
            }
        }
    }

    let resp = req
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({"text": message}))
        .send()
        .await?;

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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

            // Verify JSON body contains text field with message
            assert!(request.contains("\"text\":\"test alert\""));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result = send_webhook_notification(&url, "", "{}", "test alert").await;
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

        let result = send_webhook_notification(&url, "PUT", "{}", "test alert").await;
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

        let result = send_webhook_notification(&url, "PATCH", "{}", "test alert").await;
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

        let headers = r#"{"X-Custom":"myvalue","Authorization":"Bearer abc123"}"#;
        let result = send_webhook_notification(&url, "POST", headers, "test alert").await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_connection_refused() {
        let result =
            send_webhook_notification("http://127.0.0.1:1", "POST", "{}", "test alert").await;
        assert!(result.is_err());
    }
}
