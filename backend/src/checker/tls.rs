use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::checker::{
    cause_from_dns, cause_from_io, status_for, CheckOutcome, Checker, FailureCause,
};
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
        Self::check_all(monitor).await
    }
}

/// Why the body of a TLS check failed, so a reaching failure can be told apart
/// from a handshake one.
///
/// The two failure kinds are kept apart on purpose: a refused TCP connection
/// and a failed handshake are different causes even though both end in `down`.
enum TlsFailure {
    /// Reaching the endpoint failed (socket-level error).
    Connect(std::io::Error),
    /// The endpoint was reached but the TLS handshake failed.
    Handshake(std::io::Error),
}

/// Failure outcome built from its cause, so the status always follows the shared
/// `status_for` taxonomy.
fn failure(elapsed: u64, cause: FailureCause, message: String) -> CheckOutcome {
    CheckOutcome {
        status: status_for(Some(cause)).into(),
        status_code: None,
        response_time_ms: elapsed,
        error_message: Some(message),
        cause: Some(cause),
        ..Default::default()
    }
}

impl TlsChecker {
    /// Check body with injectable name resolution.
    ///
    /// Thin wrapper over the production loop with a single-address resolver.
    #[cfg(test)]
    pub(crate) async fn check_with<F, Fut>(monitor: &Monitor, resolve: F) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<std::net::IpAddr, crate::dns::DnsError>>,
    {
        Self::check_with_all(monitor, move |host| async move {
            resolve(host).await.map(|ip| vec![ip])
        })
        .await
    }

    /// Multi-address variant: every resolved address is attempted until one
    /// completes the TLS handshake.
    ///
    /// A stale A record must not hide a healthy one. If every address fails, the
    /// first cause is kept and the message lists each attempt.
    pub(crate) async fn check_with_all<F, Fut>(monitor: &Monitor, resolve_all: F) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<std::net::IpAddr>, crate::dns::DnsError>>,
    {
        Self::check_with_all_attempt(monitor, resolve_all, |host, ip, port, addr| async move {
            Self::attempt(&host, ip, port, &addr).await
        })
        .await
    }

    /// Same as [`Self::check_with_all`] with an injectable handshake attempt,
    /// so the fallback loop can be tested without a real TLS server.
    async fn check_with_all_attempt<F, Fut, A, AFut>(
        monitor: &Monitor,
        resolve_all: F,
        attempt: A,
    ) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<std::net::IpAddr>, crate::dns::DnsError>>,
        A: Fn(String, std::net::IpAddr, u16, String) -> AFut,
        AFut: std::future::Future<Output = Result<TlsInfo, TlsFailure>>,
    {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(monitor.timeout_seconds as u64);

        let (host, port) = parse_target(&monitor.target).unwrap_or((monitor.target.clone(), 443));
        let addr = format!("{}:{}", host, port);

        let ips = match tokio::time::timeout(timeout, resolve_all(host.clone())).await {
            Ok(Ok(ips)) if !ips.is_empty() => ips,
            Ok(Ok(_)) => {
                return failure(
                    start.elapsed().as_millis() as u64,
                    FailureCause::Other,
                    format!("DNS resolution returned no addresses for {}", host),
                );
            }
            Ok(Err(e)) => {
                tracing::warn!(target = %addr, host = %host, error = %e, "TLS check: DNS failed");
                return failure(
                    start.elapsed().as_millis() as u64,
                    cause_from_dns(&e),
                    format!("DNS resolution failed for {}: {}", host, e),
                );
            }
            Err(_) => {
                tracing::warn!(target = %addr, host = %host, "TLS check: DNS timed out");
                return failure(
                    start.elapsed().as_millis() as u64,
                    FailureCause::DnsTimeout,
                    format!("DNS resolution timed out for {}", host),
                );
            }
        };

        let mut first_cause: Option<FailureCause> = None;
        let mut attempts: Vec<String> = Vec::new();

        for ip in &ips {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                first_cause.get_or_insert(FailureCause::ConnectTimeout);
                attempts.push(format!("{}: timeout", ip));
                continue;
            }

            match tokio::time::timeout(remaining, attempt(host.clone(), *ip, port, addr.clone()))
                .await
            {
                Ok(Ok(info)) => {
                    return success_outcome(monitor, info, start.elapsed().as_millis() as u64);
                }
                Ok(Err(TlsFailure::Connect(e))) => {
                    first_cause.get_or_insert(cause_from_io(&e));
                    attempts.push(format!("{}: {}", ip, e));
                }
                Ok(Err(TlsFailure::Handshake(e))) => {
                    first_cause.get_or_insert(FailureCause::TlsError);
                    attempts.push(format!("{}: handshake failed: {}", ip, e));
                }
                Err(_) => {
                    first_cause.get_or_insert(FailureCause::ConnectTimeout);
                    attempts.push(format!("{}: timeout", ip));
                }
            }
        }

        let cause = first_cause.unwrap_or(FailureCause::Other);
        let detail = attempts.join("; ");
        let message = if attempts.len() == 1 {
            format!("TLS check failed: {detail}")
        } else {
            format!("TLS check failed for all addresses: {detail}")
        };
        failure(
            start.elapsed().as_millis() as u64,
            cause,
            crate::checker::truncate_message(message),
        )
    }

    /// Production entry: resolve every address and fall back between them.
    pub(crate) async fn check_all(monitor: &Monitor) -> CheckOutcome {
        Self::check_with_all(monitor, |host| async move {
            crate::dns::resolve_all(&host).await
        })
        .await
    }

    /// One TLS handshake attempt against a concrete address.
    ///
    /// The connector (root store + client config) is built only after the TCP
    /// connection succeeds: a refused connection must not need a crypto provider
    /// installed, and the successful address builds it exactly once per check.
    async fn attempt(
        host: &str,
        ip: std::net::IpAddr,
        port: u16,
        addr: &str,
    ) -> Result<TlsInfo, TlsFailure> {
        let tcp = tokio::net::TcpStream::connect(std::net::SocketAddr::new(ip, port))
            .await
            .map_err(|e| {
                tracing::warn!(target = %addr, error = %e, "TLS: TCP connect failed");
                TlsFailure::Connect(e)
            })?;

        let server_name =
            rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| {
                tracing::warn!(host = %host, "TLS: invalid server name");
                TlsFailure::Handshake(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid host: {}", e),
                ))
            })?;

        let connector = build_connector();
        let tls = connector.connect(server_name, tcp).await.map_err(|e| {
            tracing::warn!(target = %addr, error = %e, "TLS: handshake failed");
            TlsFailure::Handshake(e)
        })?;

        let (_, session) = tls.get_ref();
        let certs = session.peer_certificates().unwrap_or_default();

        Ok(if let Some(cert) = certs.first() {
            parse_cert_not_after(cert.as_ref())
        } else {
            TlsInfo::default()
        })
    }
}

/// Build the TLS connector once per check (root store + client config).
fn build_connector() -> tokio_rustls::TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    tokio_rustls::TlsConnector::from(Arc::new(config))
}

/// Build a successful outcome, downgrading to `warning` when the certificate
/// expires within the configured threshold.
fn success_outcome(monitor: &Monitor, info: TlsInfo, elapsed: u64) -> CheckOutcome {
    let days_left = info.cert_days_left;
    let mut outcome = CheckOutcome {
        status: status_for(None).into(),
        status_code: None,
        response_time_ms: elapsed,
        error_message: None,
        tls: Some(info),
        cause: None,
    };

    if let Some(days_left) = days_left {
        let threshold = monitor
            .config_json
            .get("expiry_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(14);

        if days_left < threshold {
            outcome.status = "warning".into();
            outcome.error_message = Some(format!("Certificate expires in {} days", days_left));
        }
    }

    outcome
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

    /// Monitor fixture for TLS checks.
    fn make_monitor(target: &str) -> Monitor {
        Monitor {
            id: "tls_test".into(),
            name: "tls test".into(),
            monitor_type: "tls".into(),
            target: target.into(),
            config_json: serde_json::json!({}),
            interval_seconds: 300,
            timeout_seconds: 5,
            enabled: true,
            notifier_id: None,
            confirmations_required: 0,
            failed_checks: 0,
            latency_threshold_ms: None,
            message_template_down: None,
            message_template_latency: None,
            message_template_up: None,
            message_template_expiry: None,
            tags: vec![],
            token: None,
            grace_seconds: None,
            last_seen_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    /// The build links two rustls providers (`ring` from our direct deps and
    /// `aws-lc-rs` through reqwest), so rustls cannot pick one by itself and
    /// `ClientConfig::builder()` panics. Checkers only need *a* provider; a
    /// deployment must select one the same way (out of scope of this change).
    fn install_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

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
        let m = make_monitor("127.0.0.1:1");
        let outcome = TlsChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
        assert!(outcome.error_message.is_some());
    }

    #[tokio::test]
    async fn test_tls_check_timeout() {
        // A resolution that never answers: the monitor timeout is what ends the
        // attempt, and it is a *resolution* timeout, not a connection one.
        // Injected, so it is deterministic and needs no network.
        let mut m = make_monitor("slow.test:443");
        m.timeout_seconds = 1;

        let started = std::time::Instant::now();
        let outcome = TlsChecker::check_with(&m, |_host| async {
            std::future::pending::<Result<std::net::IpAddr, crate::dns::DnsError>>().await
        })
        .await;

        assert_eq!(outcome.status, "error");
        assert_eq!(
            outcome.cause,
            Some(crate::checker::FailureCause::DnsTimeout)
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the monitor timeout must end the attempt, took {:?}",
            started.elapsed()
        );
    }

    /// Scenario: el check TLS usa el resolvedor propio.
    ///
    /// The hostname does not exist in the public DNS, so the only way the
    /// handshake can start is by connecting to the IP served by our cache. The
    /// listener is closed without TLS, so the check itself cannot succeed: the
    /// accepted connection is the assertion.
    #[tokio::test]
    async fn test_tls_check_resolves_with_own_resolver() {
        install_crypto_provider();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (hit_tx, hit_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let _ = hit_tx.send(());
                drop(stream);
            }
        });

        crate::dns::resolver().prime_cache(
            "watchbeat-cache.test",
            "127.0.0.1".parse().unwrap(),
            Duration::from_secs(60),
        );

        let m = make_monitor(&format!("watchbeat-cache.test:{}", port));
        let outcome = TlsChecker.check(&m).await;

        assert!(
            hit_rx.await.is_ok(),
            "the TLS check must connect to the IP from our resolver"
        );
        assert_ne!(
            outcome.status, "up",
            "a listener without TLS cannot complete the handshake"
        );
    }

    /// A name that does not exist is the target's fault, so it is reported as
    /// `down` (unlike a resolver timeout, which is `error`).
    #[tokio::test]
    async fn test_tls_check_reports_down_when_dns_nxdomain() {
        let m = make_monitor("missing.test:443");

        let outcome =
            TlsChecker::check_with(&m, |_host| async { Err(crate::dns::DnsError::Nxdomain) }).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(
            outcome.cause,
            Some(crate::checker::FailureCause::DnsNxdomain)
        );
        assert!(outcome
            .error_message
            .expect("DNS failures must carry a message")
            .contains("DNS"));
    }

    // ───── Failure cause taxonomy (Fase 2, robust check engine) ─────

    /// Scenario: Error TLS.
    #[tokio::test]
    async fn tls_check_should_report_tls_error_cause() {
        install_crypto_provider();

        // A plain TCP listener closing right away: the handshake cannot succeed.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                drop(stream);
            }
        });

        crate::dns::resolver().prime_cache(
            "watchbeat-tls-cause.test",
            "127.0.0.1".parse().unwrap(),
            Duration::from_secs(60),
        );

        let m = make_monitor(&format!("watchbeat-tls-cause.test:{port}"));
        let outcome = TlsChecker.check(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(crate::checker::FailureCause::TlsError));
        // B1: TLS derives its status the same way HTTP and TCP do.
        assert_eq!(outcome.status, crate::checker::status_for(outcome.cause));
    }

    /// The typed DNS failure survives into the TLS outcome.
    #[tokio::test]
    async fn tls_check_should_report_dns_cause() {
        let m = make_monitor("missing.test:443");

        let outcome = TlsChecker::check_with(
            &m,
            crate::checker::dns_failure(crate::dns::DnsError::Nxdomain),
        )
        .await;

        assert_eq!(outcome.status, "down");
        assert_eq!(
            outcome.cause,
            Some(crate::checker::FailureCause::DnsNxdomain)
        );
    }

    /// Scenario: TLS con dos A, una muerta y una viva.
    #[tokio::test]
    async fn tls_check_should_fall_back_to_the_next_address() {
        use std::net::{IpAddr, Ipv4Addr};

        let m = make_monitor("multi.test:443");
        let dead = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
        let alive = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        let resolve = move |_host: String| async move { Ok(vec![dead, alive]) };
        let attempt = |_host: String, ip: IpAddr, _port: u16, _addr: String| async move {
            if ip.to_string() == "127.0.0.2" {
                Err(TlsFailure::Connect(std::io::Error::from(
                    std::io::ErrorKind::ConnectionRefused,
                )))
            } else {
                Ok(TlsInfo {
                    cert_expires_at: Some("2030-01-01".into()),
                    cert_days_left: Some(1200),
                })
            }
        };

        let outcome = TlsChecker::check_with_all_attempt(&m, resolve, attempt).await;

        assert_eq!(outcome.status, "up");
        assert!(outcome.cause.is_none());
    }

    /// Scenario: todas las direcciones fallan.
    #[tokio::test]
    async fn tls_check_should_fail_when_every_address_fails() {
        use std::net::{IpAddr, Ipv4Addr};

        let m = make_monitor("multi.test:443");
        let a = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
        let b = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 3));

        let resolve = move |_host: String| async move { Ok(vec![a, b]) };
        let attempt = |_host: String, _ip: IpAddr, _port: u16, _addr: String| async move {
            Err::<TlsInfo, _>(TlsFailure::Connect(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            )))
        };

        let outcome = TlsChecker::check_with_all_attempt(&m, resolve, attempt).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::ConnectRefused));
        assert!(outcome
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("all addresses"));
    }

    /// Production path (`check_all`) against a plain TCP server: the real
    /// `attempt` runs, the connector is built after connect and the handshake
    /// failure is classified as `tls_error`.
    #[tokio::test]
    async fn production_tls_check_all_should_report_handshake_error() {
        install_crypto_provider();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                drop(stream);
            }
        });

        let m = make_monitor(&format!("127.0.0.1:{port}"));
        let outcome = TlsChecker::check_all(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::TlsError));
    }
}
