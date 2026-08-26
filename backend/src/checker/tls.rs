use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::checker::{CheckOutcome, Checker};
use crate::models::Monitor;

/// TLS certificate expiry checker.
/// Connects to host:port, performs a TLS handshake, and reads the
/// certificate's `not_after` date.
pub struct TlsChecker;

/// Result of a TLS check, including certificate info.
#[derive(Debug, Clone, Default)]
pub struct TlsInfo {
    pub cert_expires_at: Option<String>,
    pub cert_days_left: Option<i64>,
}

#[async_trait]
impl Checker for TlsChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(monitor.timeout_seconds as u64);

        let (host, port) = parse_target(&monitor.target).unwrap_or((monitor.target.clone(), 443));
        let addr = format!("{}:{}", host, port);

        let result = tokio::time::timeout(timeout, async {
            let tcp = tokio::net::TcpStream::connect(&addr).await.map_err(|e| {
                tracing::warn!(target = %addr, error = %e, "TLS: TCP connect failed");
                e
            })?;

            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

            let config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();

            let server_name =
                rustls::pki_types::ServerName::try_from(host.clone()).map_err(|e| {
                    tracing::warn!(host = %host, "TLS: invalid server name");
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid host: {}", e),
                    )
                })?;

            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let tls = connector.connect(server_name, tcp).await.map_err(|e| {
                tracing::warn!(target = %addr, error = %e, "TLS: handshake failed");
                e
            })?;

            // Extract peer certificates after handshake
            let (_, session) = tls.get_ref();
            let certs = session.peer_certificates().unwrap_or_default();

            // Parse first cert's not_after
            let info = if let Some(cert) = certs.first() {
                parse_cert_not_after(cert.as_ref())
            } else {
                TlsInfo::default()
            };

            Ok::<_, std::io::Error>(info)
        })
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(info)) => {
                let mut outcome = CheckOutcome {
                    status: "up".into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: None,
                    tls: Some(info.clone()),
                };

                // If cert expires soon, mark as warning
                if let Some(days_left) = info.cert_days_left {
                    let threshold = monitor
                        .config_json
                        .get("tls_expiry_days")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(14);

                    if days_left < threshold {
                        outcome.status = "warning".into();
                        outcome.error_message =
                            Some(format!("Certificate expires in {} days", days_left));
                    }
                }

                outcome
            }
            Ok(Err(e)) => {
                tracing::warn!(target = %addr, host = %host, error = %e, "TLS check failed");
                CheckOutcome {
                    status: "down".into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: Some(format!("TLS handshake failed: {}", e)),
                    ..Default::default()
                }
            }
            Err(_) => {
                tracing::warn!(target = %addr, host = %host, "TLS check timed out");
                CheckOutcome {
                    status: "down".into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: Some("TLS connection timed out".into()),
                    ..Default::default()
                }
            }
        }
    }
}

/// Parse a DER certificate and extract `not_after`.
pub(crate) fn parse_cert_not_after(cert_der: &[u8]) -> TlsInfo {
    use x509_parser::prelude::*;

    match X509Certificate::from_der(cert_der) {
        Ok((_, x509)) => {
            let not_after = x509.validity().not_after;
            // Convert to unix timestamp, then format via chrono
            let not_after_utc = not_after.to_datetime();
            let not_after_ts = not_after_utc.unix_timestamp();
            let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(not_after_ts, 0)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default();

            let now_ts = chrono::Utc::now().timestamp();
            let days_left = Some((not_after_ts - now_ts) / 86400);

            TlsInfo {
                cert_expires_at: Some(expires_at),
                cert_days_left: days_left,
            }
        }
        Err(_) => TlsInfo::default(),
    }
}

/// Parse "host", "host:port", "https://host", or "https://host:port/path" into (host, port).
pub fn parse_target(target: &str) -> Option<(String, u16)> {
    let trimmed = target.trim();
    // Strip URL scheme prefix if present
    let stripped = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    // Strip path after first slash
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    if let Some((host, port)) = host_port.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return Some((host.to_string(), port));
        }
    }
    Some((host_port.to_string(), 443))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target_with_port() {
        assert_eq!(
            parse_target("example.com:8443"),
            Some(("example.com".into(), 8443))
        );
    }

    #[test]
    fn test_parse_target_default_port() {
        assert_eq!(
            parse_target("example.com"),
            Some(("example.com".into(), 443))
        );
    }

    #[test]
    fn test_parse_target_port_brackets() {
        assert_eq!(parse_target("[::1]:8443"), Some(("[::1]".into(), 8443)));
    }

    #[test]
    fn test_parse_target_without_port() {
        assert_eq!(
            parse_target("example.com"),
            Some(("example.com".into(), 443))
        );
    }

    #[test]
    fn test_parse_target_ip_with_port() {
        assert_eq!(parse_target("1.1.1.1:8443"), Some(("1.1.1.1".into(), 8443)));
    }

    #[test]
    fn test_parse_target_https_url() {
        assert_eq!(
            parse_target("https://atareao.es"),
            Some(("atareao.es".into(), 443))
        );
    }

    #[test]
    fn test_parse_target_https_url_with_port() {
        assert_eq!(
            parse_target("https://atareao.es:8443"),
            Some(("atareao.es".into(), 8443))
        );
    }

    #[test]
    fn test_parse_target_https_url_with_path() {
        assert_eq!(
            parse_target("https://atareao.es/some/path"),
            Some(("atareao.es".into(), 443))
        );
    }

    #[test]
    fn test_parse_target_http_url() {
        assert_eq!(
            parse_target("http://example.com:8080"),
            Some(("example.com".into(), 8080))
        );
    }

    #[tokio::test]
    async fn test_tls_check_connection_refused() {
        let m = Monitor {
            id: "tls_test".into(),
            name: "tls test".into(),
            monitor_type: "tls".into(),
            target: "127.0.0.1:1".into(),
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
        };
        let outcome = TlsChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
        assert!(outcome.error_message.is_some());
    }

    #[tokio::test]
    async fn test_tls_check_timeout() {
        let m = Monitor {
            id: "tls_timeout".into(),
            name: "tls timeout".into(),
            monitor_type: "tls".into(),
            target: "203.0.113.1:443".into(),
            config_json: serde_json::json!({}),
            interval_seconds: 300,
            timeout_seconds: 1,
            enabled: true,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            tags: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let outcome = TlsChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
    }
}
