use async_trait::async_trait;
use serde::Serialize;
use std::sync::{Arc, OnceLock};

use crate::models::Monitor;

/// Build a `reqwest::Client` wired to the given DNS resolver.
///
/// Keeping the client construction in one place is what makes the checkers use
/// the embedded resolver (and therefore the TTL cache) instead of the OS one.
pub(crate) fn build_http_client(
    resolver: Arc<dyn reqwest::dns::Resolve>,
) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .dns_resolver(resolver)
        .redirect(reqwest::redirect::Policy::limited(5))
        .tls_info(true)
        .pool_max_idle_per_host(2)
        .build()
}

/// Shared reqwest::Client reused across all HTTP checks (created once)
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        build_http_client(crate::dns::reqwest_resolver()).expect("Failed to build reqwest::Client")
    })
}

/// Split a `host:port` target into its parts.
///
/// IPv6 must be bracketed (`[::1]:8080`): a bare `::1` is indistinguishable
/// from `host:port`, so it is rejected instead of being silently mis-parsed.
pub(crate) fn split_host_port(target: &str) -> Option<(String, u16)> {
    let target = target.trim();

    if let Some(bracketed) = target.strip_prefix('[') {
        let (host, port) = bracketed.split_once("]:")?;
        // Inside brackets only an IPv6 literal is meaningful.
        host.parse::<std::net::Ipv6Addr>().ok()?;
        return Some((host.to_string(), port.parse().ok()?));
    }

    let (host, port) = target.rsplit_once(':')?;

    if host.is_empty() || host.contains(':') {
        return None;
    }

    Some((host.to_string(), port.parse().ok()?))
}

/// Deterministic name-resolution failure, so checkers can be exercised without
/// any DNS traffic.
#[cfg(test)]
pub(crate) fn dns_failure(
    error: crate::dns::DnsError,
) -> impl FnOnce(String) -> std::future::Ready<Result<std::net::IpAddr, crate::dns::DnsError>> {
    move |_host| std::future::ready(Err(error))
}

/// Why a check failed.
///
/// Wire contract: `snake_case`, persisted as `checks.error_kind` and exposed by
/// the API. Serialization only: nothing parses this back, so the variant set is
/// the single source of truth and no unvalidated input can create one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCause {
    /// The name does not exist (NXDOMAIN).
    DnsNxdomain,
    /// Name resolution did not answer in time.
    DnsTimeout,
    /// The DNS server failed to answer the query.
    DnsServfail,
    /// The destination refused the connection.
    ConnectRefused,
    /// The connection did not complete within the monitor timeout.
    ConnectTimeout,
    /// The network (or host) is unreachable.
    NetworkUnreachable,
    /// The TLS handshake failed.
    TlsError,
    /// The endpoint answered with an unexpected HTTP status.
    HttpStatus,
    /// The response body did not match `expected_body`.
    BodyMismatch,
    /// Anything not covered by the taxonomy.
    Other,
}

impl FailureCause {
    /// Stable name of the cause, identical to its serialized form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DnsNxdomain => "dns_nxdomain",
            Self::DnsTimeout => "dns_timeout",
            Self::DnsServfail => "dns_servfail",
            Self::ConnectRefused => "connect_refused",
            Self::ConnectTimeout => "connect_timeout",
            Self::NetworkUnreachable => "network_unreachable",
            Self::TlsError => "tls_error",
            Self::HttpStatus => "http_status",
            Self::BodyMismatch => "body_mismatch",
            Self::Other => "other",
        }
    }
}

// ───── Error → cause mapping (D3) ─────

/// The one place where a cause becomes an observable status.
///
/// Unifying it is what makes HTTP, TCP and TLS coherent: the same cause always
/// yields the same status, whichever checker produced it.
///
/// `down` means the target is to blame; `error` means we could not establish
/// anything, it is on our side, or it is ambiguous. Network health attribution
/// (and the `indeterminate` status) belongs to a later phase.
pub fn status_for(cause: Option<FailureCause>) -> &'static str {
    match cause {
        None => "up",
        Some(
            FailureCause::DnsNxdomain
            | FailureCause::ConnectRefused
            | FailureCause::TlsError
            | FailureCause::HttpStatus
            | FailureCause::BodyMismatch,
        ) => "down",
        Some(
            FailureCause::DnsTimeout
            | FailureCause::DnsServfail
            | FailureCause::ConnectTimeout
            | FailureCause::NetworkUnreachable
            | FailureCause::Other,
        ) => "error",
    }
}

/// Map a typed resolution failure to its cause.
pub(crate) fn cause_from_dns(error: &crate::dns::DnsError) -> FailureCause {
    match error {
        crate::dns::DnsError::Nxdomain => FailureCause::DnsNxdomain,
        crate::dns::DnsError::Timeout => FailureCause::DnsTimeout,
        crate::dns::DnsError::Servfail => FailureCause::DnsServfail,
        // The taxonomy has no "any other DNS failure" bucket: `Other` is the
        // documented catch-all for `DnsError::Other`.
        crate::dns::DnsError::Other(_) => FailureCause::Other,
    }
}

/// Map a connectivity `io::Error` to its cause.
pub(crate) fn cause_from_io(error: &std::io::Error) -> FailureCause {
    match error.kind() {
        std::io::ErrorKind::ConnectionRefused => FailureCause::ConnectRefused,
        std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::HostUnreachable => {
            FailureCause::NetworkUnreachable
        }
        std::io::ErrorKind::TimedOut => FailureCause::ConnectTimeout,
        _ => FailureCause::Other,
    }
}

/// Walk an error chain looking for the `io::Error` underneath the wrappers.
fn io_error<'a>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a std::io::Error> {
    let mut current = Some(error);
    while let Some(err) = current {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            return Some(io);
        }
        current = err.source();
    }
    None
}

/// Classify a failed request from its error chain.
///
/// The `is_timeout`/`is_connect` flags come from `reqwest::Error`, so this stays
/// testable without building one (every branch is unit-tested).
///
/// Only a *connect* timeout is `connect_timeout`: a response timeout (the
/// connection was established but the server never answered) is ambiguous and
/// therefore `other`.
fn classify_error(
    error: &(dyn std::error::Error + 'static),
    is_timeout: bool,
    is_connect: bool,
    https: bool,
) -> FailureCause {
    if is_timeout {
        return if is_connect {
            FailureCause::ConnectTimeout
        } else {
            FailureCause::Other
        };
    }

    if let Some(io) = io_error(error) {
        let cause = cause_from_io(io);
        if cause != FailureCause::Other {
            return cause;
        }
    }

    // A connect failure on an HTTPS endpoint that is not a plain socket error
    // is a handshake problem.
    if https && is_connect {
        FailureCause::TlsError
    } else {
        FailureCause::Other
    }
}

/// Adapter over [`classify_error`] for a real `reqwest::Error`.
fn classify_request_error(error: &reqwest::Error, https: bool) -> FailureCause {
    classify_error(error, error.is_timeout(), error.is_connect(), https)
}

/// Result of a single check.
#[derive(Debug, Clone, Default)]
pub struct CheckOutcome {
    pub status: String,
    pub status_code: Option<u16>,
    pub response_time_ms: u64,
    pub error_message: Option<String>,
    pub tls: Option<crate::checker::tls::TlsInfo>,
    pub cause: Option<FailureCause>,
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
        "heartbeat" => None,
        _ => None,
    }
}

// ───── HTTP Checker ─────

pub struct HttpChecker;

/// Host of an HTTP target, when the URL can be parsed at all.
fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()?
        .host_str()
        .map(str::to_string)
}

impl HttpChecker {
    /// Check body with injectable name resolution (D2).
    ///
    /// Resolving first is what makes DNS failures exact: they are classified
    /// from the typed `DnsError` instead of being inferred from reqwest's
    /// version-dependent error strings. The lookup is nearly free thanks to the
    /// resolver cache and only runs for hostnames (never for IP literals).
    pub(crate) async fn check_with<F, Fut>(monitor: &Monitor, resolve: F) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<std::net::IpAddr, crate::dns::DnsError>>,
    {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(monitor.timeout_seconds as u64);
        let url = monitor.target.clone();
        let https = url.starts_with("https://");

        // The pre-resolve is a *classification probe*: it tells a DNS failure
        // apart from a connectivity one. The actual resolution is still done by
        // reqwest's `dns_resolver` (cached by our resolver, so it is nearly
        // free). It only runs for hostnames, never for IP literals.
        if let Some(host) = host_of(&url) {
            if host.parse::<std::net::IpAddr>().is_err() {
                let elapsed = || start.elapsed().as_millis() as u64;

                match tokio::time::timeout(timeout, resolve(host.clone())).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        let cause = cause_from_dns(&e);
                        return CheckOutcome {
                            status: status_for(Some(cause)).into(),
                            status_code: None,
                            response_time_ms: elapsed(),
                            error_message: Some(format!(
                                "DNS resolution failed for {}: {}",
                                host, e
                            )),
                            tls: None,
                            cause: Some(cause),
                        };
                    }
                    Err(_) => {
                        let cause = FailureCause::DnsTimeout;
                        return CheckOutcome {
                            status: status_for(Some(cause)).into(),
                            status_code: None,
                            response_time_ms: elapsed(),
                            error_message: Some(format!("DNS resolution timed out for {}", host)),
                            tls: None,
                            cause: Some(cause),
                        };
                    }
                }
            }
        }

        let client = http_client();

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

        // One budget for the whole check: the pre-resolve above already spent
        // part of it, so the request only gets what is left over.
        let remaining = timeout.saturating_sub(start.elapsed());
        let req = req.header("User-Agent", "WatchBeat/0.1").timeout(remaining);
        let result = req.send().await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(resp) => {
                let status_code = resp.status().as_u16();

                // Extract TLS certificate info if the URL is HTTPS
                let tls_info = if https {
                    resp.extensions()
                        .get::<reqwest::tls::TlsInfo>()
                        .and_then(|info| info.peer_certificate())
                        .map(crate::checker::tls::parse_cert_not_after)
                } else {
                    None
                };

                let expected = monitor
                    .config_json
                    .get("expected_status")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(200) as u16;

                let status_ok = (200..400).contains(&status_code) || status_code == expected;

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
                        let status = status_for(None);
                        CheckOutcome {
                            status: status.into(),
                            status_code: Some(status_code),
                            response_time_ms: elapsed,
                            error_message: None,
                            tls: tls_info.clone(),
                            cause: None,
                        }
                    } else {
                        let cause = FailureCause::BodyMismatch;
                        CheckOutcome {
                            status: status_for(Some(cause)).into(),
                            status_code: Some(status_code),
                            response_time_ms: elapsed,
                            error_message: Some("Body content did not match expected".into()),
                            tls: tls_info.clone(),
                            cause: Some(cause),
                        }
                    }
                } else {
                    let cause = FailureCause::HttpStatus;
                    CheckOutcome {
                        status: status_for(Some(cause)).into(),
                        status_code: Some(status_code),
                        response_time_ms: elapsed,
                        error_message: Some(format!("Unexpected HTTP status: {}", status_code)),
                        tls: tls_info.clone(),
                        cause: Some(cause),
                    }
                }
            }
            Err(e) => {
                let cause = classify_request_error(&e, https);
                CheckOutcome {
                    status: status_for(Some(cause)).into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: Some(format!("Request failed: {}", e)),
                    tls: None,
                    cause: Some(cause),
                }
            }
        }
    }
}

#[async_trait]
impl Checker for HttpChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        Self::check_with(
            monitor,
            |host| async move { crate::dns::resolve(&host).await },
        )
        .await
    }
}

// ───── TCP Checker ─────

pub struct TcpChecker;

impl TcpChecker {
    /// Outcome for a failed check, built from its cause so the status can never
    /// drift from the taxonomy.
    fn failure(start: std::time::Instant, cause: FailureCause, message: String) -> CheckOutcome {
        CheckOutcome {
            status: status_for(Some(cause)).into(),
            status_code: None,
            response_time_ms: start.elapsed().as_millis() as u64,
            error_message: Some(message),
            tls: None,
            cause: Some(cause),
        }
    }

    /// Check body with injectable name resolution.
    ///
    /// Resolution and connection share the monitor timeout: a slow resolver
    /// must not push the check past its budget.
    pub(crate) async fn check_with<F, Fut>(monitor: &Monitor, resolve: F) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<std::net::IpAddr, crate::dns::DnsError>>,
    {
        Self::check_with_connect(monitor, resolve, |addr| async move {
            tokio::net::TcpStream::connect(addr).await.map(|_| ())
        })
        .await
    }

    /// Check body with both the resolver and the connector injectable, so every
    /// connectivity cause can be classified deterministically (no network).
    pub(crate) async fn check_with_connect<F, Fut, C, CFut>(
        monitor: &Monitor,
        resolve: F,
        connect: C,
    ) -> CheckOutcome
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<std::net::IpAddr, crate::dns::DnsError>>,
        C: FnOnce(std::net::SocketAddr) -> CFut,
        CFut: std::future::Future<Output = std::io::Result<()>>,
    {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(monitor.timeout_seconds as u64);

        let Some((host, port)) = split_host_port(&monitor.target) else {
            return Self::failure(
                start,
                FailureCause::Other,
                format!("Invalid TCP target: {}", monitor.target),
            );
        };

        // Resolution and connection share one budget, but are timed apart: that
        // is what tells a resolution timeout from a connection one. Keeping the
        // typed `DnsError` (instead of flattening it to a string) is what
        // preserves the exact cause.
        let ip = match tokio::time::timeout(timeout, resolve(host.clone())).await {
            Ok(Ok(ip)) => ip,
            Ok(Err(e)) => {
                return Self::failure(
                    start,
                    cause_from_dns(&e),
                    format!("DNS resolution failed for {}: {}", host, e),
                );
            }
            Err(_) => {
                return Self::failure(
                    start,
                    FailureCause::DnsTimeout,
                    format!("DNS resolution timed out for {}", host),
                );
            }
        };

        let remaining = timeout.saturating_sub(start.elapsed());

        match tokio::time::timeout(remaining, connect(std::net::SocketAddr::new(ip, port))).await {
            Ok(Ok(())) => CheckOutcome {
                status: status_for(None).into(),
                status_code: None,
                response_time_ms: start.elapsed().as_millis() as u64,
                error_message: None,
                tls: None,
                cause: None,
            },
            Ok(Err(e)) => Self::failure(
                start,
                cause_from_io(&e),
                format!("TCP connection failed: {}", e),
            ),
            Err(_) => Self::failure(
                start,
                FailureCause::ConnectTimeout,
                "TCP connection timed out".into(),
            ),
        }
    }
}

#[async_trait]
impl Checker for TcpChecker {
    async fn check(&self, monitor: &Monitor) -> CheckOutcome {
        Self::check_with(
            monitor,
            |host| async move { crate::dns::resolve(&host).await },
        )
        .await
    }
}

// ───── Ping Checker ─────

/// Parse the round-trip time from `ping`'s stdout.
///
/// Handles the English (`time=`) and Spanish (`tiempo=`) markers, and the
/// sub-millisecond form (`time<1 ms`), which reports `0`.
pub(crate) fn parse_ping_time(stdout: &str) -> Option<u64> {
    for line in stdout.lines() {
        for marker in ["time=", "tiempo="] {
            if let Some(rest) = line.split(marker).nth(1) {
                if let Some(token) = rest.split_whitespace().next() {
                    let value = token.strip_suffix("ms").unwrap_or(token);
                    if let Ok(ms) = value.parse::<f64>() {
                        return Some(ms as u64);
                    }
                }
            }
        }
        if line.contains("time<") || line.contains("tiempo<") {
            return Some(0);
        }
    }
    None
}

/// Classify a failed `ping` invocation from its stderr and exit code.
///
/// Returns the status plus an actionable message. Permission problems and a
/// missing binary are configuration errors (`error`); everything else is a
/// target failure (`down`). The real stderr is always preserved.
pub(crate) fn classify_ping_failure(
    stderr: &str,
    exit_code: Option<i32>,
) -> (&'static str, String) {
    let lower = stderr.to_ascii_lowercase();
    let exit = exit_code
        .map(|code| format!(" (exit code {code})"))
        .unwrap_or_default();

    if lower.contains("permission denied") || lower.contains("operation not permitted") {
        return (
            "error",
            format!(
                "ping lacks CAP_NET_RAW: {}{}. Add \'cap_add: [NET_RAW]\' to the container or grant cap_net_raw to the ping binary.",
                stderr.trim(),
                exit
            ),
        );
    }

    if exit_code == Some(127)
        || lower.contains("command not found")
        || lower.contains("no such file")
    {
        return (
            "error",
            format!(
                "ping binary is not available: {}{}. Install iputils in the image.",
                stderr.trim(),
                exit
            ),
        );
    }

    if stderr.trim().is_empty() {
        ("down", format!("Ping failed (no response){exit}"))
    } else {
        ("down", format!("Ping failed: {}{exit}", stderr.trim()))
    }
}

/// Classify a failure to *spawn* the ping process (I/O error), as opposed to a
/// failed run. The typed `io::ErrorKind` avoids depending on localized messages.
pub(crate) fn classify_ping_spawn_error(error: &std::io::Error) -> (&'static str, String) {
    match error.kind() {
        std::io::ErrorKind::NotFound => (
            "error",
            "ping binary is not available. Install iputils in the image.".to_string(),
        ),
        std::io::ErrorKind::PermissionDenied => (
            "error",
            format!(
                "ping could not be executed: {error}. Check the binary permissions (execute bit and CAP_NET_RAW)."
            ),
        ),
        _ => (
            "error",
            format!("ping could not be started: {error}"),
        ),
    }
}

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
                .arg("--")
                .arg(&target)
                .output(),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match output {
            Ok(Ok(out)) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let time_ms = parse_ping_time(&stdout).unwrap_or(elapsed);

                CheckOutcome {
                    status: "up".into(),
                    status_code: None,
                    response_time_ms: time_ms,
                    error_message: None,
                    tls: None,
                    cause: None,
                }
            }
            Ok(Ok(out)) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let (status, message) = classify_ping_failure(&stderr, out.status.code());

                CheckOutcome {
                    status: status.into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: Some(message),
                    tls: None,
                    cause: None,
                }
            }
            Ok(Err(e)) => {
                // The binary could not be spawned at all (e.g. not installed).
                let (status, message) = classify_ping_spawn_error(&e);

                CheckOutcome {
                    status: status.into(),
                    status_code: None,
                    response_time_ms: elapsed,
                    error_message: Some(message),
                    tls: None,
                    cause: None,
                }
            }
            Err(_) => CheckOutcome {
                status: "down".into(),
                status_code: None,
                response_time_ms: timeout * 1000,
                error_message: Some("Ping timed out".into()),
                tls: None,
                cause: None,
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
        // Nothing listens on the unused port, so the connect is refused
        // immediately: deterministic without touching any external network.
        let port = closed_loopback_port().await;
        let m = make_monitor("http", &format!("http://127.0.0.1:{port}/"));

        let outcome = HttpChecker.check(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::ConnectRefused));
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

    // ───── HTTP expected_status tests ─────

    #[tokio::test]
    async fn test_http_check_with_expected_status() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({ "expected_status": 201 });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            stream
                .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
        assert_eq!(outcome.status_code, Some(201));
    }

    #[tokio::test]
    async fn test_http_check_with_wrong_expected_status() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({ "expected_status": 200 });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
        assert!(outcome.error_message.is_some());
    }

    // ───── HTTP expected_body tests ─────

    #[tokio::test]
    async fn test_http_check_with_expected_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({ "expected_body": "world" });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = "hello world";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
    }

    #[tokio::test]
    async fn test_http_check_with_wrong_expected_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({ "expected_body": "world" });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = "hello";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
        assert!(outcome.error_message.is_some());
    }

    #[tokio::test]
    async fn test_http_check_with_regex_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({
            "expected_body": "error.*timeout",
            "body_is_regex": true
        });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = "error: timeout";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
    }

    // ───── HTTP HEAD method test ─────

    #[tokio::test]
    async fn test_http_check_head_method() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.config_json = serde_json::json!({ "method": "HEAD" });

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        let outcome = HttpChecker.check(&m).await;
        assert_eq!(outcome.status, "up");
    }

    // ───── HTTP timeout test ─────

    #[tokio::test]
    async fn test_http_check_timeout() {
        // Listener accepts but never writes — client will timeout
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut m = make_monitor("http", &format!("http://{}/test", addr));
        m.timeout_seconds = 1;

        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            // Never write — client will timeout waiting for HTTP response
            let () = std::future::pending().await;
        });

        let outcome = HttpChecker.check(&m).await;
        // The connection was established, so this is *not* a connect timeout:
        // a slow server is ambiguous (`other`), not the target refusing us.
        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::Other));
        assert!(outcome.response_time_ms < 5000);
    }

    // ───── TCP connection refused test ─────

    #[tokio::test]
    async fn test_tcp_check_connection_refused() {
        let m = make_monitor("tcp", "127.0.0.1:1");
        let outcome = TcpChecker.check(&m).await;
        assert_eq!(outcome.status, "down");
        assert!(outcome.error_message.is_some());
    }

    // ───── TCP timeout test ─────

    #[tokio::test]
    async fn test_tcp_check_timeout() {
        // The connector never answers, so the monitor timeout is what ends the
        // attempt. Injected, so it is deterministic and needs no network.
        let mut m = make_monitor("tcp", "127.0.0.1:80");
        m.timeout_seconds = 1;

        let connect = |_addr: std::net::SocketAddr| async {
            std::future::pending::<std::io::Result<()>>().await
        };

        let started = std::time::Instant::now();
        let outcome = TcpChecker::check_with_connect(
            &m,
            |host| async move { crate::dns::resolve(&host).await },
            connect,
        )
        .await;

        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::ConnectTimeout));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "the monitor timeout must end the attempt, took {:?}",
            started.elapsed()
        );
    }

    // ───── Own resolver integration ─────

    #[test]
    fn test_split_host_port_with_hostname() {
        assert_eq!(
            split_host_port("example.com:8443"),
            Some(("example.com".into(), 8443))
        );
    }

    #[test]
    fn test_split_host_port_with_bracketed_ipv6() {
        assert_eq!(split_host_port("[::1]:8080"), Some(("::1".into(), 8080)));
    }

    #[test]
    fn test_split_host_port_without_port() {
        assert_eq!(split_host_port("example.com"), None);
    }

    #[test]
    fn test_split_host_port_rejects_bare_ipv6_without_brackets() {
        assert_eq!(split_host_port("::1"), None);
        assert_eq!(split_host_port("2001:db8::1"), None);
        assert_eq!(split_host_port("[]:8080"), None);
    }

    /// Scenario: Check TCP usa el resolvedor propio.
    ///
    /// The hostname does not exist in the public DNS, so reaching the local
    /// listener is only possible through the IP obtained from our resolver.
    #[tokio::test]
    async fn test_tcp_check_uses_ip_from_own_resolver() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
        });

        crate::dns::resolver().prime_cache(
            "watchbeat-cache.test",
            "127.0.0.1".parse().unwrap(),
            std::time::Duration::from_secs(60),
        );

        let m = make_monitor("tcp", &format!("watchbeat-cache.test:{}", port));
        let outcome = TcpChecker.check(&m).await;

        assert_eq!(outcome.status, "up");
    }

    #[tokio::test]
    async fn test_tcp_check_with_invalid_target() {
        let m = make_monitor("tcp", "no-port-here");
        let outcome = TcpChecker.check(&m).await;

        assert_eq!(outcome.status, "error");
        assert!(outcome.error_message.is_some());
    }

    /// A hanging DNS resolution must not outlive the monitor timeout: failure
    /// to resolve is part of the check budget, not an extra unbounded delay.
    #[tokio::test]
    async fn test_tcp_check_timeout_covers_dns_resolution() {
        let mut m = make_monitor("tcp", "slow.test:80");
        m.timeout_seconds = 1;

        let resolve = |_host: String| async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            Err(crate::dns::DnsError::Timeout)
        };

        let started = std::time::Instant::now();
        let outcome = TcpChecker::check_with(&m, resolve).await;

        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::DnsTimeout));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "resolution must be cut by the monitor timeout, took {:?}",
            started.elapsed()
        );
    }

    /// A name that does not exist is the target's fault, so it is reported as
    /// `down` (unlike a resolver timeout, which is `error`).
    #[tokio::test]
    async fn test_tcp_check_reports_down_when_dns_nxdomain() {
        let m = make_monitor("tcp", "missing.test:80");

        let outcome =
            TcpChecker::check_with(&m, |_host| async { Err(crate::dns::DnsError::Nxdomain) }).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::DnsNxdomain));
        assert!(outcome
            .error_message
            .expect("DNS failures must carry a message")
            .contains("DNS"));
    }

    // ───── Failure cause taxonomy (Fase 2, robust check engine) ─────

    /// Deterministic connectivity failure, so connectivity causes can be
    /// classified without touching a socket.
    fn io_failure(kind: std::io::ErrorKind) -> std::io::Error {
        std::io::Error::new(kind, format!("injected {kind:?}"))
    }

    /// `rustls` gets both `ring` and `aws-lc-rs` in this tree, so the
    /// process-level provider is ambiguous and the tests must install one
    /// explicitly (pre-existing issue, see the Fase 1 notes).
    fn install_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    // Named builders for the DNS variants: a `fn` pointer keeps the test tables
    // readable without tripping `clippy::type_complexity`.
    type DnsCase = (&'static str, fn() -> crate::dns::DnsError, FailureCause);
    type DnsErrorCase = (&'static str, fn() -> crate::dns::DnsError);

    fn nxdomain() -> crate::dns::DnsError {
        crate::dns::DnsError::Nxdomain
    }

    fn dns_timeout() -> crate::dns::DnsError {
        crate::dns::DnsError::Timeout
    }

    fn servfail() -> crate::dns::DnsError {
        crate::dns::DnsError::Servfail
    }

    fn dns_other() -> crate::dns::DnsError {
        crate::dns::DnsError::Other("boom".into())
    }

    /// A loopback port with nothing listening on it, so a connect attempt is
    /// refused immediately by the kernel.
    ///
    /// Port 1 is privileged and outside the ephemeral range, so `bind(:0)` can
    /// never hand it to another parallel test. The previous bind-and-release of
    /// an ephemeral port raced with other tests: they could receive the same
    /// port and start listening between the drop and the connect, turning an
    /// expected `down` into `up`.
    async fn closed_loopback_port() -> u16 {
        1
    }

    /// The helper must not race with parallel `bind(:0)` calls: a port outside
    /// the ephemeral range is never handed out to another test, so nothing can
    /// start listening between the drop and the connect.
    #[tokio::test]
    async fn closed_loopback_port_should_be_deterministic_and_refused() {
        let port = closed_loopback_port().await;

        assert!(
            port < 32768,
            "port {port} must be outside the ephemeral range (32768-60999)"
        );

        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        let error = tokio::net::TcpStream::connect(addr)
            .await
            .expect_err("nothing must be listening on the closed port");
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionRefused);
    }

    /// One-shot HTTP server answering `status_line` + `body`, so status and body
    /// classification can be tested without leaving the loopback interface.
    async fn spawn_http_server(
        status_line: &'static str,
        body: &'static str,
    ) -> std::net::SocketAddr {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = format!(
                "{status_line}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });

        addr
    }

    // ── The single cause→status table (architecture decision) ──

    /// Every cause has exactly one status, and `None` means `up`. This is the
    /// source of truth all three checkers derive their failures from.
    #[test]
    fn status_for_should_map_every_cause_to_its_status() {
        let table = [
            (None, "up"),
            // The target is to blame.
            (Some(FailureCause::DnsNxdomain), "down"),
            (Some(FailureCause::ConnectRefused), "down"),
            (Some(FailureCause::TlsError), "down"),
            (Some(FailureCause::HttpStatus), "down"),
            (Some(FailureCause::BodyMismatch), "down"),
            // We could not establish anything, or it is our side / ambiguous.
            (Some(FailureCause::DnsTimeout), "error"),
            (Some(FailureCause::DnsServfail), "error"),
            (Some(FailureCause::ConnectTimeout), "error"),
            (Some(FailureCause::NetworkUnreachable), "error"),
            (Some(FailureCause::Other), "error"),
        ];

        for (cause, expected) in table {
            assert_eq!(
                status_for(cause),
                expected,
                "cause {cause:?} must map to {expected}"
            );
        }
    }

    // ── Error→cause classification, without network (B2) ──

    #[test]
    fn cause_from_io_should_classify_connectivity_errors() {
        assert_eq!(
            cause_from_io(&io_failure(std::io::ErrorKind::ConnectionRefused)),
            FailureCause::ConnectRefused
        );
        assert_eq!(
            cause_from_io(&io_failure(std::io::ErrorKind::TimedOut)),
            FailureCause::ConnectTimeout
        );
        assert_eq!(
            cause_from_io(&io_failure(std::io::ErrorKind::NetworkUnreachable)),
            FailureCause::NetworkUnreachable
        );
        assert_eq!(
            cause_from_io(&io_failure(std::io::ErrorKind::HostUnreachable)),
            FailureCause::NetworkUnreachable
        );
        assert_eq!(
            cause_from_io(&io_failure(std::io::ErrorKind::BrokenPipe)),
            FailureCause::Other
        );
    }

    /// A response timeout is not a connect timeout (M1), and the HTTPS/keyword
    /// branches are pinned without needing a real `reqwest::Error`.
    #[test]
    fn classify_error_should_classify_every_branch() {
        let plain = io_failure(std::io::ErrorKind::Other);

        // A connect timeout is ours-ish/ambiguous but at least about connecting.
        assert_eq!(
            classify_error(&plain, true, true, false),
            FailureCause::ConnectTimeout
        );
        // Timeout after the connection was up (slow server): ambiguous.
        assert_eq!(
            classify_error(&plain, true, false, false),
            FailureCause::Other
        );
        assert_eq!(
            classify_error(
                &io_failure(std::io::ErrorKind::ConnectionRefused),
                false,
                true,
                false
            ),
            FailureCause::ConnectRefused
        );
        assert_eq!(
            classify_error(
                &io_failure(std::io::ErrorKind::NetworkUnreachable),
                false,
                true,
                false
            ),
            FailureCause::NetworkUnreachable
        );
        // HTTPS + connect failure without a socket error: the handshake.
        assert_eq!(
            classify_error(&plain, false, true, true),
            FailureCause::TlsError
        );
        // Anything else.
        assert_eq!(
            classify_error(&plain, false, true, false),
            FailureCause::Other
        );
        assert_eq!(
            classify_error(&plain, false, false, false),
            FailureCause::Other
        );
    }

    // ── Cause classification through the checkers ──

    /// Scenario: Dominio inexistente.
    #[tokio::test]
    async fn http_check_should_report_dns_nxdomain_cause() {
        let m = make_monitor("http", "http://missing.test/");

        let outcome =
            HttpChecker::check_with(&m, dns_failure(crate::dns::DnsError::Nxdomain)).await;

        assert_eq!(outcome.cause, Some(FailureCause::DnsNxdomain));
        assert_eq!(outcome.status, "down");
    }

    /// The three checkers are unified: the same cause always yields the same
    /// status, because all of them go through `status_for`.
    #[tokio::test]
    async fn checkers_should_agree_on_status_per_dns_cause() {
        let cases: [DnsCase; 3] = [
            ("nxdomain", nxdomain, FailureCause::DnsNxdomain),
            ("timeout", dns_timeout, FailureCause::DnsTimeout),
            ("servfail", servfail, FailureCause::DnsServfail),
        ];

        for (name, dns_error, cause) in cases {
            let expected = status_for(Some(cause));

            let http = HttpChecker::check_with(
                &make_monitor("http", "http://missing.test/"),
                dns_failure(dns_error()),
            )
            .await;
            let tcp = TcpChecker::check_with(
                &make_monitor("tcp", "missing.test:80"),
                dns_failure(dns_error()),
            )
            .await;
            let tls = crate::checker::tls::TlsChecker::check_with(
                &make_monitor("tls", "missing.test:443"),
                dns_failure(dns_error()),
            )
            .await;

            for (checker, outcome) in [("http", &http), ("tcp", &tcp), ("tls", &tls)] {
                assert_eq!(
                    outcome.cause,
                    Some(cause),
                    "{checker} must report cause {name}"
                );
                assert_eq!(
                    outcome.status, expected,
                    "{checker} must map {name} to {expected}, got {}",
                    outcome.status
                );
            }

            assert_eq!(
                (http.status, tcp.status, tls.status),
                (
                    expected.to_string(),
                    expected.to_string(),
                    expected.to_string()
                ),
                "every checker must agree on the status of {name}"
            );
        }
    }

    /// Scenario: Conexión rechazada, en los tres checkers (M2, B1).
    #[tokio::test]
    async fn checkers_should_agree_on_status_for_connect_refused() {
        let port = closed_loopback_port().await;

        let http = HttpChecker
            .check(&make_monitor("http", &format!("http://127.0.0.1:{port}/")))
            .await;
        let tcp = TcpChecker
            .check(&make_monitor("tcp", &format!("127.0.0.1:{port}")))
            .await;
        // The connect fails before any handshake, so no crypto provider needed.
        let tls = crate::checker::tls::TlsChecker
            .check(&make_monitor("tls", &format!("127.0.0.1:{port}")))
            .await;

        for (checker, outcome) in [("http", &http), ("tcp", &tcp), ("tls", &tls)] {
            assert_eq!(
                outcome.cause,
                Some(FailureCause::ConnectRefused),
                "{checker} must classify a refused connection"
            );
            assert_eq!(outcome.status, "down", "{checker} must report `down`");
        }
    }

    /// Scenario: Timeout de resolución de nombres.
    #[tokio::test]
    async fn http_check_should_report_dns_timeout_cause() {
        let m = make_monitor("http", "http://slow.test/");

        let outcome = HttpChecker::check_with(&m, dns_failure(crate::dns::DnsError::Timeout)).await;

        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::DnsTimeout));
    }

    /// Scenario: Fallo del servidor DNS.
    #[tokio::test]
    async fn http_check_should_report_dns_servfail_cause() {
        let m = make_monitor("http", "http://broken.test/");

        let outcome =
            HttpChecker::check_with(&m, dns_failure(crate::dns::DnsError::Servfail)).await;

        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::DnsServfail));
    }

    /// Scenario: La causa se propaga en el resultado del check.
    ///
    /// TCP keeps the typed `DnsError`, so the exact variant survives into
    /// `CheckOutcome` instead of being flattened into a message.
    #[tokio::test]
    async fn tcp_check_should_report_typed_dns_cause() {
        let m = make_monitor("tcp", "broken.test:443");

        let outcome = TcpChecker::check_with(&m, dns_failure(crate::dns::DnsError::Servfail)).await;

        assert_eq!(outcome.status, "error");
        assert_eq!(outcome.cause, Some(FailureCause::DnsServfail));
    }

    /// Scenario: Conexión rechazada.
    #[tokio::test]
    async fn tcp_check_should_report_connect_refused_cause() {
        // Port 1 has no listener, so the kernel resets the SYN.
        let port = closed_loopback_port().await;

        let m = make_monitor("tcp", &format!("127.0.0.1:{port}"));
        let outcome = TcpChecker.check(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::ConnectRefused));
    }

    /// Scenario: Timeout de conexión (determinista, con el connector inyectado).
    #[tokio::test]
    async fn tcp_check_should_report_connect_timeout_cause() {
        let m = make_monitor("tcp", "127.0.0.1:80");

        let connect = |_addr: std::net::SocketAddr| async {
            Err::<(), _>(io_failure(std::io::ErrorKind::TimedOut))
        };
        let outcome = TcpChecker::check_with_connect(
            &m,
            |host| async move { crate::dns::resolve(&host).await },
            connect,
        )
        .await;

        assert_eq!(outcome.cause, Some(FailureCause::ConnectTimeout));
        assert_eq!(outcome.status, "error");
    }

    /// Scenario: Red inalcanzable (determinista, sin rutas IPv6).
    #[tokio::test]
    async fn tcp_check_should_report_network_unreachable_cause() {
        let m = make_monitor("tcp", "127.0.0.1:80");

        let connect = |_addr: std::net::SocketAddr| async {
            Err::<(), _>(io_failure(std::io::ErrorKind::NetworkUnreachable))
        };
        let outcome = TcpChecker::check_with_connect(
            &m,
            |host| async move { crate::dns::resolve(&host).await },
            connect,
        )
        .await;

        assert_eq!(outcome.cause, Some(FailureCause::NetworkUnreachable));
        assert_eq!(outcome.status, "error");
    }

    /// An unknown connectivity error falls back to `other` (and still `error`).
    #[tokio::test]
    async fn tcp_check_should_report_other_cause_for_unknown_connect_errors() {
        let m = make_monitor("tcp", "127.0.0.1:80");

        let connect = |_addr: std::net::SocketAddr| async {
            Err::<(), _>(io_failure(std::io::ErrorKind::BrokenPipe))
        };
        let outcome = TcpChecker::check_with_connect(
            &m,
            |host| async move { crate::dns::resolve(&host).await },
            connect,
        )
        .await;

        assert_eq!(outcome.cause, Some(FailureCause::Other));
        assert_eq!(outcome.status, "error");
    }

    /// Scenario: Status HTTP inesperado.
    #[tokio::test]
    async fn http_check_should_report_http_status_cause() {
        let addr = spawn_http_server("HTTP/1.1 404 Not Found", "").await;
        let mut m = make_monitor("http", &format!("http://{addr}/"));
        m.config_json = serde_json::json!({ "expected_status": 200 });

        let outcome = HttpChecker.check(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::HttpStatus));
    }

    /// Scenario: Contenido no esperado.
    #[tokio::test]
    async fn http_check_should_report_body_mismatch_cause() {
        let addr = spawn_http_server("HTTP/1.1 200 OK", "hello").await;
        let mut m = make_monitor("http", &format!("http://{addr}/"));
        m.config_json = serde_json::json!({ "expected_body": "world" });

        let outcome = HttpChecker.check(&m).await;

        assert_eq!(outcome.status, "down");
        assert_eq!(outcome.cause, Some(FailureCause::BodyMismatch));
    }

    /// Scenario: Check correcto sin causa (HTTP).
    #[tokio::test]
    async fn http_check_should_report_no_cause_when_up() {
        let addr = spawn_http_server("HTTP/1.1 200 OK", "ok").await;
        let m = make_monitor("http", &format!("http://{addr}/"));

        let outcome = HttpChecker.check(&m).await;

        assert_eq!(outcome.status, "up");
        assert_eq!(outcome.cause, None);
        assert_eq!(status_for(outcome.cause), "up");
    }

    /// Scenario: Check correcto sin causa (TCP).
    #[tokio::test]
    async fn tcp_check_should_report_no_cause_when_up() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let m = make_monitor("tcp", &addr.to_string());
        let outcome = TcpChecker.check(&m).await;

        assert_eq!(outcome.status, "up");
        assert_eq!(outcome.cause, None);
    }

    /// `ping` is outside the taxonomy: whatever the outcome, it carries no cause.
    #[tokio::test]
    async fn ping_check_should_report_no_cause() {
        let m = make_monitor("ping", "127.0.0.1");

        let outcome = PingChecker.check(&m).await;

        assert!(
            outcome.cause.is_none(),
            "ping is outside the taxonomy, got {:?}",
            outcome.cause
        );
    }

    /// Scenario: la conexión rechazada se clasifica por el camino real de
    /// reqwest (M2), sin red externa: el puerto del loopback no tiene nadie
    /// escuchando, así que el kernel rechaza el connect de inmediato.
    #[tokio::test]
    async fn http_check_should_classify_a_refused_connection() {
        let port = closed_loopback_port().await;
        let m = make_monitor("http", &format!("http://127.0.0.1:{port}/"));

        let outcome = HttpChecker.check(&m).await;

        assert_eq!(outcome.cause, Some(FailureCause::ConnectRefused));
        assert_eq!(outcome.status, status_for(outcome.cause));
        assert_eq!(outcome.status, "down");
    }

    /// M3: el pre-resolve es una *sonda de clasificación*, no un presupuesto
    /// aparte. El presupuesto del monitor es único, así que un probe lento deja
    /// al request solo el tiempo restante (antes se sumaban los dos).
    #[tokio::test]
    async fn http_check_should_share_a_single_timeout_budget_with_the_probe() {
        use tokio::io::AsyncReadExt;

        // Acepta pero nunca responde: el request solo puede terminar por timeout.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                std::future::pending::<()>().await;
            }
        });
        // El nombre lo resuelve reqwest con nuestro resolver cacheado, así que el
        // request sí llega a nuestro listener.
        crate::dns::resolver().prime_cache(
            "watchbeat-budget.test",
            "127.0.0.1".parse().unwrap(),
            std::time::Duration::from_secs(60),
        );

        let mut m = make_monitor("http", &format!("http://watchbeat-budget.test:{port}/"));
        m.timeout_seconds = 1;

        // La sonda se come 800 ms de los 1000 ms del monitor.
        let outcome = HttpChecker::check_with(&m, |_host| async {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        })
        .await;

        assert_ne!(
            outcome.status, "up",
            "a server that never answers cannot be up"
        );
        assert_eq!(outcome.status, status_for(outcome.cause));
        assert!(
            outcome.response_time_ms < 1500,
            "sonda y request deben compartir un solo presupuesto, tardó {} ms",
            outcome.response_time_ms
        );
    }

    /// B1: los tres checkers derivan el status de `status_for`, así que no
    /// pueden discrepar sobre una misma causa. Cubre las 10 causas.
    #[tokio::test]
    async fn checkers_should_derive_status_from_status_for() {
        let mut outcomes: Vec<(String, CheckOutcome)> = Vec::new();

        // Causas DNS, en los tres checkers.
        let dns_cases: [DnsErrorCase; 4] = [
            ("dns_nxdomain", nxdomain),
            ("dns_timeout", dns_timeout),
            ("dns_servfail", servfail),
            ("dns_other", dns_other),
        ];
        for (name, error) in dns_cases {
            outcomes.push((
                format!("http/{name}"),
                HttpChecker::check_with(
                    &make_monitor("http", "http://missing.test/"),
                    dns_failure(error()),
                )
                .await,
            ));
            outcomes.push((
                format!("tcp/{name}"),
                TcpChecker::check_with(
                    &make_monitor("tcp", "missing.test:80"),
                    dns_failure(error()),
                )
                .await,
            ));
            outcomes.push((
                format!("tls/{name}"),
                crate::checker::tls::TlsChecker::check_with(
                    &make_monitor("tls", "missing.test:443"),
                    dns_failure(error()),
                )
                .await,
            ));
        }

        // Causas de conectividad, con el connector inyectado (sin red).
        let connect_cases: [(&str, std::io::ErrorKind); 3] = [
            ("connect_timeout", std::io::ErrorKind::TimedOut),
            (
                "network_unreachable",
                std::io::ErrorKind::NetworkUnreachable,
            ),
            ("other", std::io::ErrorKind::BrokenPipe),
        ];
        for (name, kind) in connect_cases {
            let outcome = TcpChecker::check_with_connect(
                &make_monitor("tcp", "127.0.0.1:80"),
                |host| async move { crate::dns::resolve(&host).await },
                move |_addr| async move { Err::<(), _>(io_failure(kind)) },
            )
            .await;
            outcomes.push((format!("tcp/{name}"), outcome));
        }

        // Refused, en HTTP (ruta real de reqwest) y TCP.
        let port = closed_loopback_port().await;
        outcomes.push((
            "http/connect_refused".into(),
            HttpChecker
                .check(&make_monitor("http", &format!("http://127.0.0.1:{port}/")))
                .await,
        ));
        outcomes.push((
            "tcp/connect_refused".into(),
            TcpChecker
                .check(&make_monitor("tcp", &format!("127.0.0.1:{port}")))
                .await,
        ));

        // Causas de protocolo (HTTP) y el caso correcto.
        let not_found = spawn_http_server("HTTP/1.1 404 Not Found", "").await;
        let mut status_monitor = make_monitor("http", &format!("http://{not_found}/"));
        status_monitor.config_json = serde_json::json!({ "expected_status": 200 });
        outcomes.push((
            "http/http_status".into(),
            HttpChecker.check(&status_monitor).await,
        ));

        let wrong_body = spawn_http_server("HTTP/1.1 200 OK", "hello").await;
        let mut body_monitor = make_monitor("http", &format!("http://{wrong_body}/"));
        body_monitor.config_json = serde_json::json!({ "expected_body": "world" });
        outcomes.push((
            "http/body_mismatch".into(),
            HttpChecker.check(&body_monitor).await,
        ));

        let healthy = spawn_http_server("HTTP/1.1 200 OK", "ok").await;
        outcomes.push((
            "http/up".into(),
            HttpChecker
                .check(&make_monitor("http", &format!("http://{healthy}/")))
                .await,
        ));

        // Causa TLS: un listener TCP plano que cierra sin handshake.
        install_crypto_provider();
        let tls_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tls_port = tls_listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((stream, _)) = tls_listener.accept().await {
                drop(stream);
            }
        });
        crate::dns::resolver().prime_cache(
            "watchbeat-equivalence.test",
            "127.0.0.1".parse().unwrap(),
            std::time::Duration::from_secs(60),
        );
        outcomes.push((
            "tls/tls_error".into(),
            crate::checker::tls::TlsChecker
                .check(&make_monitor(
                    "tls",
                    &format!("watchbeat-equivalence.test:{tls_port}"),
                ))
                .await,
        ));

        // La equivalencia no puede ser vacua: debe cubrir toda la taxonomía.
        let mut seen: Vec<FailureCause> = outcomes
            .iter()
            .filter_map(|(_, outcome)| outcome.cause)
            .collect();
        seen.sort_by_key(|cause| cause.as_str());
        seen.dedup();
        assert_eq!(
            seen.len(),
            10,
            "the equivalence test must exercise the whole taxonomy, saw {seen:?}"
        );

        for (label, outcome) in &outcomes {
            assert_eq!(
                outcome.status,
                status_for(outcome.cause),
                "{label} must derive its status from status_for (cause {:?})",
                outcome.cause
            );
        }
    }

    /// The wire contract of the taxonomy is frozen: `snake_case`, and the name
    /// exposed to the API matches the serialized one.
    #[test]
    fn failure_cause_should_serialize_in_snake_case() {
        let taxonomy = [
            (FailureCause::DnsNxdomain, "dns_nxdomain"),
            (FailureCause::DnsTimeout, "dns_timeout"),
            (FailureCause::DnsServfail, "dns_servfail"),
            (FailureCause::ConnectRefused, "connect_refused"),
            (FailureCause::ConnectTimeout, "connect_timeout"),
            (FailureCause::NetworkUnreachable, "network_unreachable"),
            (FailureCause::TlsError, "tls_error"),
            (FailureCause::HttpStatus, "http_status"),
            (FailureCause::BodyMismatch, "body_mismatch"),
            (FailureCause::Other, "other"),
        ];

        for (cause, name) in &taxonomy {
            assert_eq!(
                serde_json::to_value(cause).unwrap(),
                serde_json::json!(name)
            );
            assert_eq!(cause.as_str(), *name);
        }

        assert_eq!(taxonomy.len(), 10, "taxonomy must match the documented set");
    }

    // ───── Ping diagnostics (#31) ─────

    #[test]
    fn parse_ping_time_handles_english_and_spanish_locales() {
        assert_eq!(
            parse_ping_time("64 bytes from 8.8.8.8: icmp_seq=1 ttl=117 time=12.3 ms"),
            Some(12)
        );
        assert_eq!(
            parse_ping_time("64 bytes from 8.8.8.8: icmp_seq=1 ttl=117 tiempo=12.3 ms"),
            Some(12)
        );
        assert_eq!(
            parse_ping_time("64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time=0.025 ms"),
            Some(0)
        );
    }

    #[test]
    fn parse_ping_time_handles_sub_millisecond() {
        assert_eq!(
            parse_ping_time("64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time<1 ms"),
            Some(0)
        );
    }

    #[test]
    fn classify_ping_failure_treats_dns_failure_as_down() {
        let (status, message) =
            classify_ping_failure("ping: bad address 'does-not-exist.invalid'", Some(2));
        assert_eq!(status, "down", "a target DNS failure is not a config error");
        assert!(message.contains("bad address"));
    }

    #[test]
    fn classify_ping_spawn_error_detects_missing_binary() {
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);
        let (status, message) = classify_ping_spawn_error(&error);
        assert_eq!(status, "error");
        assert!(message.contains("iputils"));
    }

    #[test]
    fn parse_ping_time_returns_none_without_timing() {
        assert_eq!(
            parse_ping_time("PING 8.8.8.8 (8.8.8.8): 56 data bytes"),
            None
        );
        assert_eq!(parse_ping_time(""), None);
    }

    #[test]
    fn classify_ping_failure_detects_permission_denied() {
        let (status, message) =
            classify_ping_failure("ping: permission denied (are you root?)", Some(1));
        assert_eq!(status, "error");
        assert!(message.contains("permission denied"), "got: {message}");
        assert!(message.contains("CAP_NET_RAW"), "got: {message}");
    }

    #[test]
    fn classify_ping_failure_detects_operation_not_permitted() {
        let (status, message) = classify_ping_failure("ping: Operation not permitted", Some(1));
        assert_eq!(status, "error");
        assert!(message.contains("CAP_NET_RAW"), "got: {message}");
    }

    #[test]
    fn classify_ping_failure_detects_missing_binary() {
        let (status, message) = classify_ping_failure("ping: not found", Some(127));
        assert_eq!(status, "error");
        assert!(message.contains("not available"), "got: {message}");
    }

    #[test]
    fn classify_ping_failure_includes_stderr_and_exit_code() {
        let (status, message) = classify_ping_failure("ping: bad address 'nope'", Some(2));
        assert_eq!(status, "down");
        assert!(message.contains("bad address 'nope'"), "got: {message}");
        assert!(message.contains("exit code 2"), "got: {message}");
    }

    #[test]
    fn classify_ping_failure_without_stderr_reports_no_response() {
        let (status, message) = classify_ping_failure("", Some(1));
        assert_eq!(status, "down");
        assert!(message.contains("no response"), "got: {message}");
        assert!(message.contains("exit code 1"), "got: {message}");
    }
}
