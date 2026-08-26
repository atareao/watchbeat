pub async fn send_ntfy_notification(
    topic: &str,
    server_url: &str,
    token: Option<&str>,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/{}", server_url.trim_end_matches('/'), topic);

    let mut req = reqwest::Client::new().post(&url).body(message.to_string());
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ntfy API error: {} — {}", status, body);
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

            // Verify body contains the message
            assert!(request.contains("my test message"));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result =
            send_ntfy_notification("mytopic", &server_url, None, "my test message").await;
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
                request
                    .to_lowercase()
                    .contains("authorization: bearer my_secret_token"),
                "Missing Bearer auth:\n{}",
                request
            );

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result = send_ntfy_notification(
            "mytopic",
            &server_url,
            Some("my_secret_token"),
            "my test message",
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

        let result =
            send_ntfy_notification("mytopic", &server_url, None, "my test message").await;
        assert!(result.is_ok());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_ntfy_connection_refused() {
        let result =
            send_ntfy_notification("topic", "http://127.0.0.1:1", None, "test message")
                .await;
        assert!(result.is_err());
    }
}
