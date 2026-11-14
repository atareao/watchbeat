//! Canary for the health of our own outbound connectivity (`NetworkHealth`).
//!
//! Phase 3 of PLAN-002. The canary answers one question: *is this failure the
//! target's fault, or ours?* It probes the host's egress **without depending on
//! the system resolver** (TCP connects addressed by IP literal) plus one name
//! resolution through our own resolver (phase 1) to measure the DNS path
//! specifically.
//!
//! Design: `openspec/changes/network-attribution/design.md` (D1–D4).

use std::any::Any;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::FutureExt;

/// Default period between canary cycles (D1), derived from
/// [`DEFAULT_INTERVAL_SECS`] so both can never diverge.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(DEFAULT_INTERVAL_SECS);

/// Per-probe deadline. A probe that does not answer in time counts as a failure.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Smallest accepted period between canary cycles. `tokio::time::interval`
/// panics on a zero period, so a misconfigured interval is clamped to this (M6b).
pub const MIN_INTERVAL: Duration = Duration::from_millis(1);

/// Setting key for the configurable canary period.
pub const SETTING_CANARY_INTERVAL_SECS: &str = "canary_interval_secs";
/// Default canary period in seconds when the setting is absent or invalid.
pub const DEFAULT_INTERVAL_SECS: u64 = 30;
/// Smallest configurable canary period, in seconds (matches PROBE_TIMEOUT).
pub const MIN_INTERVAL_SECS: u64 = 5;
/// Largest configurable canary period, in seconds.
pub const MAX_INTERVAL_SECS: u64 = 3600;

/// DNS servers probed by IP literal: the system resolver is never involved (D2).
const TCP_TARGETS: [&str; 3] = ["1.1.1.1:53", "8.8.8.8:53", "9.9.9.9:53"];

/// Well-known name resolved through our own resolver to measure the DNS path (D2).
const DNS_PROBE_HOST: &str = "cloudflare.com";

/// Health of the host's own outbound connectivity (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkHealth {
    /// No cycle has run yet: no information.
    Unknown,
    /// Every probe answered.
    Healthy,
    /// Egress works but name resolution does not.
    DnsDegraded,
    /// No TCP probe answered at all.
    Down,
}

impl NetworkHealth {
    /// Stable name, used for logs and the wire contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::DnsDegraded => "dns_degraded",
            Self::Down => "down",
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Healthy,
            2 => Self::DnsDegraded,
            3 => Self::Down,
            _ => Self::Unknown,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Healthy => 1,
            Self::DnsDegraded => 2,
            Self::Down => 3,
        }
    }
}

/// Clonable, `Send + Sync` publication point for the canary's last value (D4).
///
/// [`NetworkHealthHandle::current`] never blocks and never awaits: the scheduler
/// reads it on every check without paying for a lock.
#[derive(Clone)]
pub struct NetworkHealthHandle {
    state: Arc<AtomicU8>,
    /// Effective canary period, published to the running task so a new setting
    /// is adopted without restarting the process. Cloning the sender clones the
    /// set: every clone sees the same period.
    interval_tx: tokio::sync::watch::Sender<Duration>,
}

impl NetworkHealthHandle {
    /// Create a handle that reports [`NetworkHealth::Unknown`] until the first
    /// cycle, and whose period is [`DEFAULT_INTERVAL`].
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(NetworkHealth::Unknown.as_u8())),
            interval_tx: tokio::sync::watch::channel(DEFAULT_INTERVAL).0,
        }
    }

    /// Last published health. Non-blocking, never awaits.
    pub fn current(&self) -> NetworkHealth {
        NetworkHealth::from_u8(self.state.load(Ordering::Relaxed))
    }

    /// Publish a new value. Called by the canary task once per cycle.
    pub fn publish(&self, health: NetworkHealth) {
        self.state.store(health.as_u8(), Ordering::Relaxed);
    }

    /// Change the canary period so the running canary adopts it.
    ///
    /// The value is clamped to the accepted setting range
    /// (`[MIN_INTERVAL_SECS, MAX_INTERVAL_SECS]`), so a value coming from the
    /// `canary_interval_secs` setting behaves exactly like
    /// [`interval_from_setting`]. A task that is already running wakes up and
    /// restarts its ticker with the new period.
    pub fn set_interval(&self, interval: Duration) {
        self.interval_tx
            .send_replace(clamp_setting_interval(interval));
    }

    /// Current effective (clamped) canary period.
    pub fn interval(&self) -> Duration {
        *self.interval_tx.borrow()
    }

    /// Subscribe to period changes. Used by the canary task to adopt them.
    fn subscribe(&self) -> tokio::sync::watch::Receiver<Duration> {
        self.interval_tx.subscribe()
    }

    /// Seed the period before the task starts. Unlike [`set_interval`], it only
    /// guarantees a strictly positive period, so callers that are not the
    /// setting (tests) may use sub-second periods.
    fn seed_period(&self, interval: Duration) {
        self.interval_tx.send_replace(valid_interval(interval));
    }
}

impl Default for NetworkHealthHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// The two probes the canary needs, behind a trait so tests run without network.
#[async_trait]
pub trait Probes: Send + Sync {
    /// Open a TCP connection to `addr`. `true` when the connection succeeded.
    async fn tcp_probe(&self, addr: SocketAddr) -> bool;

    /// Resolve `host` through our own resolver. `true` when it answered.
    async fn dns_probe(&self, host: &str) -> bool;
}

/// Real probes: the host's own egress.
struct SystemProbes;

#[async_trait]
impl Probes for SystemProbes {
    async fn tcp_probe(&self, addr: SocketAddr) -> bool {
        tokio::net::TcpStream::connect(addr).await.is_ok()
    }

    async fn dns_probe(&self, host: &str) -> bool {
        crate::dns::resolve(host).await.is_ok()
    }
}

/// Derive the health from the raw probe results (D3). Pure and total.
pub fn derive_health(any_tcp_ok: bool, dns_ok: bool) -> NetworkHealth {
    match (any_tcp_ok, dns_ok) {
        (false, _) => NetworkHealth::Down,
        (true, false) => NetworkHealth::DnsDegraded,
        (true, true) => NetworkHealth::Healthy,
    }
}

/// One canary configuration: probes plus their targets.
struct Canary {
    probes: Box<dyn Probes>,
    timeout: Duration,
    tcp_targets: Vec<SocketAddr>,
    dns_host: String,
}

impl Canary {
    /// Canary wired to the real network.
    fn system() -> Self {
        Self::with_probes(Box::new(SystemProbes))
    }

    /// Canary with injected probes (tests).
    fn with_probes(probes: Box<dyn Probes>) -> Self {
        Self {
            probes,
            timeout: PROBE_TIMEOUT,
            tcp_targets: tcp_targets(),
            dns_host: DNS_PROBE_HOST.to_string(),
        }
    }

    /// Run every probe once and derive the health. A probe that exceeds the
    /// deadline counts as a failure.
    async fn run_once(&self) -> NetworkHealth {
        let mut any_tcp_ok = false;
        for addr in &self.tcp_targets {
            if probe_ok(self.timeout, self.probes.tcp_probe(*addr)).await {
                any_tcp_ok = true;
            }
        }
        let dns_ok = probe_ok(self.timeout, self.probes.dns_probe(&self.dns_host)).await;
        derive_health(any_tcp_ok, dns_ok)
    }
}

/// The TCP probe targets, as `SocketAddr`s (IP literals only — see D2).
fn tcp_targets() -> Vec<SocketAddr> {
    TCP_TARGETS
        .iter()
        .filter_map(|target| target.parse().ok())
        .collect()
}

/// Await a probe future, treating a missed deadline as a failure.
async fn probe_ok<F>(timeout: Duration, probe: F) -> bool
where
    F: std::future::Future<Output = bool>,
{
    tokio::time::timeout(timeout, probe).await.unwrap_or(false)
}

/// Clamp a configured interval to a valid, strictly positive period (M6b).
///
/// `tokio::time::interval` panics on a zero period, so a misconfigured interval
/// degrades to a fast-but-valid canary instead of taking the task down.
fn valid_interval(interval: Duration) -> Duration {
    interval.max(MIN_INTERVAL)
}

/// Clamp a setting-derived interval to the accepted range, in whole seconds.
///
/// Values below [`MIN_INTERVAL_SECS`] would hammer the egress with a token
/// canary; values above [`MAX_INTERVAL_SECS`] would leave the health stale for
/// too long.
fn clamp_setting_interval(interval: Duration) -> Duration {
    interval.clamp(
        Duration::from_secs(MIN_INTERVAL_SECS),
        Duration::from_secs(MAX_INTERVAL_SECS),
    )
}

/// Turn the raw setting value into a canary period, clamped to a sane range.
///
/// An absent or unparseable value falls back to [`DEFAULT_INTERVAL_SECS`], and
/// any accepted value is clamped to `[MIN_INTERVAL_SECS, MAX_INTERVAL_SECS]`.
pub fn interval_from_setting(value: Option<&str>) -> Duration {
    let secs = value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_SECS)
        .clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS);

    Duration::from_secs(secs)
}

/// Run one cycle, turning a panic into `None` so the task survives (M6a).
///
/// A canary that dies would freeze the health on a stale value forever, which
/// would silently break attribution.
async fn run_cycle(canary: &Canary) -> Option<NetworkHealth> {
    match AssertUnwindSafe(canary.run_once()).catch_unwind().await {
        Ok(health) => Some(health),
        Err(payload) => {
            tracing::error!(
                "netcheck: canary cycle panicked: {}",
                panic_message(payload.as_ref())
            );
            None
        }
    }
}

/// Human-readable rendering of a panic payload.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}

/// Start the canary task and return its publication handle (D1).
pub fn spawn(interval: Duration) -> NetworkHealthHandle {
    spawn_canary(interval, Canary::system())
}

/// [`spawn`] with an injected canary (tests).
fn spawn_canary(interval: Duration, canary: Canary) -> NetworkHealthHandle {
    let handle = NetworkHealthHandle::new();
    // The requested period is the one the ticker starts with; every later
    // change goes through `set_interval` (the setting path).
    handle.seed_period(interval);

    let published = handle.clone();
    let mut rx = published.subscribe();

    tokio::spawn(async move {
        let mut period = valid_interval(published.interval());
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Some(health) = run_cycle(&canary).await {
                        published.publish(health);
                        tracing::debug!(
                            health = health.as_str(),
                            "netcheck: published network health"
                        );
                    }
                }
                changed = rx.changed() => {
                    if changed.is_err() {
                        // No sender left: nothing can change the period any more,
                        // so stop probing instead of spinning on a ready channel
                        // (a `changed()` error is always immediately ready).
                        break;
                    }
                    period = valid_interval(*rx.borrow());
                    ticker =
                        tokio::time::interval_at(tokio::time::Instant::now() + period, period);
                    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                }
            }
        }
    });

    handle
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Deterministic probes: every probe answers with a fixed verdict, or hangs.
    struct FakeProbes {
        tcp_ok: bool,
        dns_ok: bool,
        hang: bool,
    }

    impl FakeProbes {
        fn new(tcp_ok: bool, dns_ok: bool) -> Self {
            Self {
                tcp_ok,
                dns_ok,
                hang: false,
            }
        }

        fn hanging() -> Self {
            Self {
                tcp_ok: true,
                dns_ok: true,
                hang: true,
            }
        }
    }

    #[async_trait]
    impl Probes for FakeProbes {
        async fn tcp_probe(&self, _addr: SocketAddr) -> bool {
            if self.hang {
                std::future::pending::<()>().await;
            }
            self.tcp_ok
        }

        async fn dns_probe(&self, _host: &str) -> bool {
            if self.hang {
                std::future::pending::<()>().await;
            }
            self.dns_ok
        }
    }

    fn canary(probes: FakeProbes) -> Canary {
        Canary::with_probes(Box::new(probes))
    }

    /// Canary over an arbitrary probe implementation.
    fn canary_boxed(probes: Box<dyn Probes>) -> Canary {
        Canary::with_probes(probes)
    }

    /// Probes that panic on their very first call, then answer normally. Used to
    /// prove the canary survives a panicking cycle (M6a).
    struct PanicOnceProbes {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Probes for PanicOnceProbes {
        async fn tcp_probe(&self, _addr: SocketAddr) -> bool {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("probe exploded");
            }
            true
        }

        async fn dns_probe(&self, _host: &str) -> bool {
            true
        }
    }

    /// Probes that count their calls and always answer healthily.
    struct CountingProbes {
        calls: AtomicUsize,
    }

    /// `Probes` on the shared pointer itself, so the counting `Arc` can be
    /// handed to the canary as a `Box<dyn Probes>` while the test keeps
    /// observing the very same counter.
    #[async_trait]
    impl Probes for Arc<CountingProbes> {
        async fn tcp_probe(&self, _addr: SocketAddr) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            true
        }

        async fn dns_probe(&self, _host: &str) -> bool {
            true
        }
    }

    // ── Scenario: salida completamente sana ──

    #[test]
    fn derive_health_should_be_healthy_when_every_probe_answers() {
        assert_eq!(derive_health(true, true), NetworkHealth::Healthy);
    }

    // ── Scenario: DNS de salida degradado ──

    #[test]
    fn derive_health_should_be_dns_degraded_when_tcp_answers_but_dns_does_not() {
        assert_eq!(derive_health(true, false), NetworkHealth::DnsDegraded);
    }

    // ── Scenario: salida caída ──

    #[test]
    fn derive_health_should_be_down_when_no_tcp_probe_answers() {
        assert_eq!(derive_health(false, true), NetworkHealth::Down);
        assert_eq!(derive_health(false, false), NetworkHealth::Down);
    }

    // ── Scenario: estado inicial sin información ──

    #[test]
    fn a_new_handle_should_report_unknown() {
        assert_eq!(NetworkHealthHandle::new().current(), NetworkHealth::Unknown);
    }

    // ── Scenario: los sondeos por IP no usan el DNS del sistema ──

    #[test]
    fn tcp_targets_should_be_ip_literals_so_the_system_dns_is_never_used() {
        let targets = tcp_targets();
        assert_eq!(targets.len(), TCP_TARGETS.len());
        for target in TCP_TARGETS {
            assert!(
                target.parse::<SocketAddr>().is_ok(),
                "{target} must be an IP:port literal, never a name"
            );
        }
    }

    // ── Scenario: un sondeo que expira cuenta como fallo ──

    #[tokio::test]
    async fn a_probe_that_times_out_should_count_as_a_failure() {
        let mut canary = canary(FakeProbes::hanging());
        canary.timeout = Duration::from_millis(10);

        assert_eq!(canary.run_once().await, NetworkHealth::Down);
    }

    // ── Cycle derivation through the probe trait ──

    #[tokio::test]
    async fn run_once_should_derive_health_from_the_probes() {
        assert_eq!(
            canary(FakeProbes::new(true, true)).run_once().await,
            NetworkHealth::Healthy
        );
        assert_eq!(
            canary(FakeProbes::new(true, false)).run_once().await,
            NetworkHealth::DnsDegraded
        );
        assert_eq!(
            canary(FakeProbes::new(false, false)).run_once().await,
            NetworkHealth::Down
        );
    }

    // ── Scenario: lectura no bloqueante del último valor ──

    #[test]
    fn handle_should_report_the_last_published_health() {
        let handle = NetworkHealthHandle::new();
        handle.publish(NetworkHealth::Healthy);
        assert_eq!(handle.current(), NetworkHealth::Healthy);
    }

    // ── Scenario: la salud se actualiza en cada ciclo ──

    #[tokio::test]
    async fn the_canary_task_should_publish_the_result_of_each_cycle() {
        let handle = spawn_canary(
            Duration::from_secs(3600),
            canary(FakeProbes::new(true, false)),
        );

        let mut health = handle.current();
        for _ in 0..1_000 {
            if health != NetworkHealth::Unknown {
                break;
            }
            tokio::task::yield_now().await;
            health = handle.current();
        }

        assert_eq!(health, NetworkHealth::DnsDegraded);
    }

    // ── M6b: el intervalo del canario debe ser válido ──

    #[test]
    fn valid_interval_should_reject_a_zero_period() {
        assert!(
            valid_interval(Duration::ZERO) > Duration::ZERO,
            "a zero period must be clamped to a strictly positive one"
        );
        assert_eq!(
            valid_interval(Duration::from_secs(30)),
            Duration::from_secs(30),
            "a valid period must be left untouched"
        );
    }

    #[tokio::test]
    async fn a_zero_interval_should_not_kill_the_canary() {
        let handle = spawn_canary(
            Duration::ZERO,
            canary_boxed(Box::new(FakeProbes::new(true, true))),
        );

        let health = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if handle.current() != NetworkHealth::Unknown {
                    break handle.current();
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("a zero interval must not panic the canary task");

        assert_eq!(health, NetworkHealth::Healthy);
    }

    // ── M6a: un ciclo con panic no detiene el canario ──

    #[tokio::test]
    async fn a_panicking_cycle_should_not_stop_the_canary() {
        let handle = spawn_canary(
            Duration::from_millis(10),
            canary_boxed(Box::new(PanicOnceProbes {
                calls: AtomicUsize::new(0),
            })),
        );

        let health = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if handle.current() != NetworkHealth::Unknown {
                    break handle.current();
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the canary stopped publishing after a panicking cycle");

        assert_eq!(
            health,
            NetworkHealth::Healthy,
            "the canary must keep publishing after a panicking cycle"
        );
    }

    // ── Scenario: periodo del canario configurable desde settings ──

    /// Default period when no setting has ever been stored.
    #[test]
    fn interval_from_setting_should_default_when_absent() {
        assert_eq!(
            interval_from_setting(None),
            Duration::from_secs(DEFAULT_INTERVAL_SECS)
        );
        assert_eq!(interval_from_setting(None), Duration::from_secs(30));
    }

    /// A plain numeric value is used as-is when inside the accepted range.
    #[test]
    fn interval_from_setting_should_parse_a_valid_value() {
        assert_eq!(
            interval_from_setting(Some("60")),
            Duration::from_secs(60),
            "a valid number of seconds must be honoured"
        );
    }

    /// Garbage never reaches the scheduler: it degrades to the default.
    #[test]
    fn interval_from_setting_should_default_on_invalid_input() {
        let expected = Duration::from_secs(DEFAULT_INTERVAL_SECS);

        assert_eq!(
            interval_from_setting(Some("abc")),
            expected,
            "a non-numeric value must fall back to the default"
        );
        assert_eq!(
            interval_from_setting(Some("")),
            expected,
            "an empty value must fall back to the default"
        );
        assert_eq!(
            interval_from_setting(Some("-5")),
            expected,
            "a negative value must fall back to the default"
        );
    }

    /// Too small a period is clamped up: a token canary would hammer the egress.
    #[test]
    fn interval_from_setting_should_clamp_to_the_minimum() {
        let floor = Duration::from_secs(MIN_INTERVAL_SECS);

        assert_eq!(
            interval_from_setting(Some("0")),
            floor,
            "zero must be clamped up to the minimum"
        );
        assert_eq!(
            interval_from_setting(Some("1")),
            floor,
            "one second must be clamped up to the minimum"
        );
    }

    /// Too large a period is clamped down.
    #[test]
    fn interval_from_setting_should_clamp_to_the_maximum() {
        assert_eq!(
            interval_from_setting(Some("99999")),
            Duration::from_secs(MAX_INTERVAL_SECS),
            "an absurdly long period must be clamped down to the maximum"
        );
    }

    // ── Scenario: el intervalo efectivo del handle ──

    /// A fresh handle must report the default period.
    #[test]
    fn a_new_handle_should_report_the_default_interval() {
        assert_eq!(
            NetworkHealthHandle::new().interval(),
            Duration::from_secs(DEFAULT_INTERVAL_SECS)
        );
    }

    /// A stored interval must be readable back.
    #[test]
    fn set_interval_should_be_visible_through_interval() {
        let handle = NetworkHealthHandle::new();
        handle.set_interval(Duration::from_secs(120));
        assert_eq!(handle.interval(), Duration::from_secs(120));
    }

    /// The handle clamps exactly like the setting parser does.
    #[test]
    fn set_interval_should_clamp_out_of_range_values() {
        let handle = NetworkHealthHandle::new();

        handle.set_interval(Duration::from_secs(0));
        assert_eq!(handle.interval(), Duration::from_secs(MIN_INTERVAL_SECS));

        handle.set_interval(Duration::from_secs(99999));
        assert_eq!(handle.interval(), Duration::from_secs(MAX_INTERVAL_SECS));
    }

    // ── Scenario: cambio del setting se aplica en caliente ──

    /// Saving `canary_interval_secs` must reach the running canary through
    /// `set_interval`, with no restart: the task re-arms its ticker and probes
    /// again. Driven by tokio's paused clock, so it is deterministic and
    /// instant instead of waiting for a real 5 s setting-minimum period.
    #[tokio::test(start_paused = true)]
    async fn adopting_a_new_interval_does_not_require_a_restart() {
        let probes = Arc::new(CountingProbes {
            calls: AtomicUsize::new(0),
        });

        // A canary whose period is so long that only the immediate first tick
        // ever fires: anything counted afterwards proves a hot reload.
        let handle = spawn_canary(
            Duration::from_secs(3600),
            canary_boxed(Box::new(Arc::clone(&probes))),
        );

        // Let the spawned task run its immediate first tick. Only `yield_now`
        // is used while waiting: it never parks the runtime, so the paused clock
        // cannot auto-advance to the far-away second tick behind our back.
        let mut before = 0;
        for _ in 0..100 {
            before = probes.calls.load(Ordering::SeqCst);
            if before > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(before > 0, "the first cycle ran no probe at all");

        // Shorten the period through the handle: this is the setting path.
        handle.set_interval(Duration::from_secs(5));

        // Let the task observe the watch change and re-arm its ticker at
        // `now + 5s`...
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }

        // ...then move the virtual clock past the adopted period.
        tokio::time::advance(Duration::from_secs(5)).await;

        let mut adopted = false;
        for _ in 0..100 {
            if probes.calls.load(Ordering::SeqCst) > before {
                adopted = true;
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            adopted,
            "the canary did not adopt the new interval without a restart"
        );
    }
}
