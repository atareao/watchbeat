use async_trait::async_trait;

use crate::models::{CheckResult, Monitor};

/// Result of a single check.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub status: String, // "up" | "down" | "error"
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
}

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

                if (200..400).contains(&status) || status == expected {
                    CheckOutcome {
                        status: "up".into(),
                        status_code: Some(status),
                        response_time_ms: elapsed,
                        error_message: None,
                    }
                } else {
                    CheckOutcome {
                        status: "down".into(),
                        status_code: Some(status),
                        response_time_ms: elapsed,
                        error_message: Some(format!("Unexpected HTTP status: {}", status)),
                    }
                }
            }
            Err(e) => CheckOutcome {
                status: if e.is_timeout() { "down" } else { "error" }.into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some(format!("Request failed: {}", e)),
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
            },
            Ok(Err(e)) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: Some(format!("TCP connection failed: {}", e)),
            },
            Err(_) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: timeout.as_millis() as u64,
                error_message: Some("TCP connection timed out".into()),
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
                .arg(&timeout.to_string())
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
                }
            }
            Ok(Ok(_out)) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some("Ping failed (no response)".into()),
            },
            Ok(Err(e)) => CheckOutcome {
                status: "error".into(),
                status_code: None,
                response_time_ms: elapsed,
                error_message: Some(format!("Ping command error: {}", e)),
            },
            Err(_) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: timeout * 1000,
                error_message: Some("Ping timed out".into()),
            },
        }
    }
}