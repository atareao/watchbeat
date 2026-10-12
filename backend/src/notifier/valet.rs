/// Send a notification to a Valet inbox via its ingest API.
///
/// `message` is truncated to a maximum of 4096 bytes before serializing
/// (Valet rejects larger payloads with a 400).
pub async fn send_valet_notification(
    base_url: &str,
    ingest_token: &str,
    origin: &str,
    author: &str,
    level: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/api/notifications", base_url.trim_end_matches('/'));

    // Truncate the message to <= 4096 bytes (JSON string). Truncate by bytes
    // but avoid splitting a multi-byte UTF-8 character mid-sequence.
    let mut truncated = message;
    if message.len() > 4096 {
        // Walk back from byte 4096 to a char boundary.
        let mut end = 4096;
        while end > 0 && !message.is_char_boundary(end) {
            end -= 1;
        }
        truncated = &message[..end];
    }

    let body = serde_json::json!({
        "message": truncated,
        "level": level,
        "origin": origin,
        "author": author,
    });

    let json = body.to_string();

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", ingest_token))
        .header("Content-Type", "application/json")
        .body(json)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Valet API error: {} — {}", status, body);
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
    async fn test_valet_sends_correct_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Verify POST method and correct path
            assert!(request.contains("POST"), "Request was: {}", request);
            assert!(
                request.contains("/api/notifications"),
                "Request was: {}",
                request
            );
            assert!(
                request
                    .to_lowercase()
                    .contains("content-type: application/json"),
                "Request was: {}",
                request
            );

            // Verify JSON body fields (parse the body after the headers)
            let body_start = request
                .find("\r\n\r\n")
                .map(|p| p + 4)
                .unwrap_or(request.len());
            let body = &request[body_start..];
            let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(
                parsed["message"], "Monitor down: X",
                "Wrong message in body: {}",
                request
            );
            assert_eq!(
                parsed["level"], "critical",
                "Wrong level in body: {}",
                request
            );
            assert_eq!(
                parsed["origin"], "WatchBeat",
                "Wrong origin in body: {}",
                request
            );
            assert_eq!(
                parsed["author"], "Servidor WEB",
                "Wrong author in body: {}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result = send_valet_notification(
            &base_url,
            "token",
            "WatchBeat",
            "Servidor WEB",
            "critical",
            "Monitor down: X",
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_valet_with_bearer_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // Verify Authorization header with Bearer token (case-insensitive)
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer my_secret_token"),
                "Missing Bearer auth:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result = send_valet_notification(
            &base_url,
            "my_secret_token",
            "WatchBeat",
            "author",
            "info",
            "hello",
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_valet_error_status() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _raw = read_http_request(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 37\r\n\r\n{\"error\":\"message is required\"}",
                )
                .await
                .unwrap();
        });

        let result =
            send_valet_notification(&base_url, "token", "WatchBeat", "author", "info", "").await;
        assert!(result.is_err());
        let err = format!("{}", result.err().unwrap());
        assert!(err.contains("Valet API error: 400"), "Error was: {}", err);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_valet_connection_refused() {
        let result = send_valet_notification(
            "http://127.0.0.1:1",
            "token",
            "WatchBeat",
            "author",
            "info",
            "hello",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_valet_truncates_long_message() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}", addr);

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let raw = read_http_request(&mut stream).await;
            let request = String::from_utf8_lossy(&raw);

            // The "message" value inside the JSON body must be truncated to
            // <= 4096 bytes (Valet rejects larger messages with a 400).
            let body_start = request
                .find("\r\n\r\n")
                .map(|p| p + 4)
                .unwrap_or(request.len());
            let body = &request[body_start..];
            let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
            let msg = parsed["message"].as_str().unwrap();
            assert!(
                msg.len() <= 4096,
                "Message not truncated: {} bytes in body: {}",
                msg.len(),
                request
            );

            stream
                .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let long_message = "x".repeat(5000);
        let result = send_valet_notification(
            &base_url,
            "token",
            "WatchBeat",
            "author",
            "info",
            &long_message,
        )
        .await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }
}
