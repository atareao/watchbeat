use async_trait::async_trait;

use crate::models::Monitor;

/// Result of a single check.
#[derive(Debug, Clone, Default)]
pub struct CheckOutcome {
    pub status: String,
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub tls: Option<crate::checker::tls::TlsInfo>,
}

pub mod tls;

/// A checker knows how to verify a specific type of endpoint.
#[async_trait]
pub trait Checker: Send + Sync {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome;
}

/// Build an appropriate Checker for the given monitor type.
pub fn checker_for(monitor: &Monitor) -> Option<Box<dyn Checker>> {
    match monitor.monitor_type.as_str() {
        "http" => Some(Box::new(HttpChecker)),
        "tcp" => Some(Box::new(TcpChecker)),
        "ping" => Some(Box::new(PingChecker)),
        "tls" => Some(Box::new(tls::TlsChecker)),
        _ => None,
    }
}

// ───── HTTP Checker ─────

pub struct HttpChecker;

#[async_trait]
impl Checker for HttpChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(monitor.timeout_seconds as u64);
        let url = monitor.target.clone();

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                return CheckOutcome {
                    status: "error".into(),
                    status_code: None,
                    response_time_ms: start.elapsed().as_millis() as u64,
                    error_message: Some(format!("Client build error: {}", e)),
                    tls: None,
                };
            }
        };

        let method = monitor
            .config_json
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let req = match method {
            "HEAD" => client.head(&url),
            "POST" => client.post(&url),
            _ => client.get(&url),
        };

        let req = req.header("User-Agent", "Vigilatrs/0.1");
        let result = req.send().await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let expected = monitor
                    .config_json
                    .get("expected_status")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(200) as u16;

                let status_ok = (200..400).contains(&status) || status == expected;

                if status_ok {
                    // Content validation: expected_body with optional regex
                    let expected_body = monitor
                        .config_json
                        .get("expected_body")
                        .and_then(|v| v.as_str());

                    let body_ok = if let Some(pattern) = expected_body {
                        let body_is_regex = monitor
                            .config_json
                            .get("body_is_regex")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // Read body (limited to 64KB)
                        let body = resp.text().await.unwrap_or_default();
                        let limited = body.chars().take(64 * 1024).collect::<String>();

                        if body_is_regex {
                            regex::Regex::new(pattern)
                                .map(|re| re.is_match(&limited))
                                .unwrap_or(false)
                        } else {
                            limited.contains(pattern)
                        }
                    } else {
                        true
                    };

                    if body_ok {
                        CheckOutcome {
                            status: "up".into(),
                            status_code: Some(status),
                            response_time_ms: elapsed,
                            error_message: None,
                            tls: None,
                        }
                    } else {
                        CheckOutcome {
                            status: "down".into(),
                            status_code: Some(status),
                            response_time_ms: elapsed,
                            error_message: Some("Body content did not match expected".into()),
                            tls: None,
                        }
                    }
                } else {
                    CheckOutcome {
                        status: "down".into(),
                        status_code: Some(status),
                        response_time_ms: elapsed,
                        error_message: Some(format!("Unexpected HTTP status: {}", status)),
                        tls: None,
                    }
                }
            }
            Err(e) => CheckOutcome {
                status: if e.is_timeout() { "down" } else { "error" }.into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some(format!("Request failed: {}", e)),
                tls: None,
            },
        }
    }
}

// ───── TCP Checker ─────

pub struct TcpChecker;

#[async_trait]
impl Checker for TcpChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(monitor.timeout_seconds as u64);

        let addr = monitor.target.clone();

        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => CheckOutcome {
                status: "up".into(),
                status_code: None,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: None,
                tls: None,
            },
            Ok(Err(e)) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: Some(format!("TCP connection failed: {}", e)),
                tls: None,
            },
            Err(_) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: timeout.as_millis() as u64,
                error_message: Some("TCP connection timed out".into()),
                tls: None,
            },
        }
    }
}

// ───── Ping Checker ─────

pub struct PingChecker;

#[async_trait]
impl Checker for PingChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        let start = std::time::Instant::now();
        let timeout = monitor.timeout_seconds as u64;

        let target = monitor.target.clone();

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("ping")
                .arg("-c")
                .arg("1")
                .arg("-W")
                .arg(timeout.to_string())
                .arg(&target)
                .output(),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match output {
            Ok(Ok(out)) if out.status.success() => {
                // Parse ping time from output
                let stdout = String::from_utf8_lossy(&out.stdout);
                let time_ms = stdout
                    .lines()
                    .find(|l| l.contains("time="))
                    .and_then(|l| {
                        l.split("time=")
                            .nth(1)
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|s| s.trim_end_matches("ms").parse::<f64>().ok())
                    })
                    .map(|t| t as u64)
                    .unwrap_or(elapsed);

                CheckOutcome {
                    status: "up".into(),
                    status_code: None,
                    response_time_ms: time_ms,
                    error_message: None,
                    tls: None,
                }
            }
            Ok(Ok(_out)) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some("Ping failed (no response)".into()),
                tls: None,
            },
            Ok(Err(e)) => CheckOutcome {
                status: "error".into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some(format!("Ping command error: {}", e)),
                tls: None,
            },
            Err(_) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: timeout * 1000,
                error_message: Some("Ping timed out".into()),
                tls: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Monitor;

    fn make_monitor(monitor_type: &str, target: &str) -> Monitor {
        Monitor {
            id: "t".into(),
            name: "test".into(),
            monitor_type: monitor_type.into(),
            target: target.into(),
            config_json: serde_json::json!({}),
            interval_seconds: 300,
            timeout_seconds: 5,
            enabled: true,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            tags: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_checker_for_http() {
        assert!(checker_for(&make_monitor("http", "https://ex.com")).is_some());
    }

    #[test]
    fn test_checker_for_tcp() {
        assert!(checker_for(&make_monitor("tcp", "localhost:8080")).is_some());
    }

    #[test]
    fn test_checker_for_ping() {
        assert!(checker_for(&make_monitor("ping", "8.8.8.8")).is_some());
    }

    #[test]
    fn test_checker_for_unknown() {
        assert!(checker_for(&make_monitor("unknown", "x")).is_none());
    }

    #[test]
    fn test_checker_for_tls() {
        assert!(checker_for(&make_monitor("tls", "example.com")).is_some());
    }

    #[tokio::test]
    async fn test_http_check_up() {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let m = make_monitor("http", &format!("http://{}/test", addr));

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
        assert!(outcome.response_time_ms < 5000);
        assert!(outcome.error_message.is_none());
    }

    #[tokio::test]
    async fn test_http_check_down() {
        let m = make_monitor("http", "http://203.0.113.1:1");
        let outcome = HttpChecker.check(&m).await;
        assert!(outcome.status == "down" || outcome.status == "error");
    }

    #[tokio::test]
    async fn test_tcp_check_up() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let m = make_monitor("tcp", &addr.to_string());

        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
        });

        let outcome = TcpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
    }
}
