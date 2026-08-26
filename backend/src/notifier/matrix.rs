/// Send a notification via Matrix. Called from the scheduler after resolving
/// notifier config from DB.
pub async fn send_matrix_notification(
    homeserver_url: &str,
    access_token: &str,
    room_id: &str,
    message: &str,
) -> anyhow::Result<()> {
    let url = format!(
        "{}/_matrix/client/v3/rooms/{}/send/m.room.message",
        homeserver_url.trim_end_matches('/'),
        room_id,
    );
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "msgtype": "m.text",
            "body": message,
        }))
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

            // Verify POST method and correct path (no txn_id suffix now)
            assert!(
                request.starts_with("POST"),
                "Expected POST method, got:\n{}",
                request
            );
            assert!(
                request.contains("/_matrix/client/v3/rooms/!room:id/send/m.room.message"),
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

            // Verify JSON body fields — just the message content
            assert!(request.contains("\"msgtype\":\"m.text\""));
            assert!(request.contains("\"body\":\"Test Alert\""));

            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await
                .unwrap();
        });

        let result = send_matrix_notification(
            &server_url,
            "test_token",
            "!room:id",
            "Test Alert",
        )
        .await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_matrix_connection_refused() {
        let result = send_matrix_notification(
            "http://127.0.0.1:1",
            "token",
            "!room:id",
            "test message",
        )
        .await;
        assert!(
            result.is_err(),
            "Expected connection error, got Ok: {:?}",
            result
        );
    }
}
