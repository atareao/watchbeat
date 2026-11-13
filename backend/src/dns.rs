//! Own DNS resolver: a TTL-aware cache in front of a pool of upstream servers.
//!
//! Why not the OS resolver? A micro-cut of the host DNS (~5 s) makes *every*
//! monitor fail at once, reporting services as `down` while they are perfectly
//! healthy. Resolving through our own cached resolver keeps the checks running
//! during such a cut: the cached IP is still valid.
//!
//! Design notes:
//! - The cache is the only cache (`hickory`'s is disabled), so `min_ttl` /
//!   `max_ttl` are the single source of truth for TTL clamping.
//! - Upstreams are tried in order with a short per-attempt timeout.
//! - HTTP checks consume this resolver through [`ReqwestDnsResolver`], so
//!   `reqwest` still sees the hostname and keeps SNI and the `Host` header.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use hickory_resolver::config::{LookupIpStrategy, NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::net::{DnsError as HickoryDnsError, NetError};
use hickory_resolver::proto::op::ResponseCode;
use hickory_resolver::proto::rr::{Name, RecordType};
use hickory_resolver::{Resolver, TokioResolver};

/// Upstream servers used when none is configured.
pub const DEFAULT_UPSTREAMS: [&str; 3] = ["1.1.1.1", "8.8.8.8", "9.9.9.9"];

/// Environment variable overriding [`DEFAULT_UPSTREAMS`] (comma separated).
pub const UPSTREAMS_ENV: &str = "WATCHBEAT_DNS_UPSTREAMS";

/// Upper bound on cached entries; expired ones are purged when it is reached.
const MAX_CACHE_ENTRIES: usize = 1024;

/// Errors surfaced by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DnsError {
    /// No upstream answered within the attempt timeout.
    #[error("DNS query timed out")]
    Timeout,
    /// An upstream answered SERVFAIL.
    #[error("DNS server returned SERVFAIL")]
    Servfail,
    /// The domain does not exist (NXDOMAIN).
    #[error("domain does not exist (NXDOMAIN)")]
    Nxdomain,
    /// Any other resolution failure.
    #[error("DNS error: {0}")]
    Other(String),
}

/// A single successful upstream answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpstreamAnswer {
    pub ip: IpAddr,
    pub ttl: Duration,
}

/// One DNS upstream.
#[async_trait::async_trait]
pub trait Upstream: Send + Sync {
    async fn query(&self, host: &str) -> Result<UpstreamAnswer, DnsError>;
}

/// Tuning knobs. `Default` yields the production values.
#[derive(Debug, Clone)]
pub struct DnsConfig {
    pub upstreams: Vec<IpAddr>,
    pub min_ttl: Duration,
    pub max_ttl: Duration,
    pub attempt_timeout: Duration,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            upstreams: default_upstreams(),
            min_ttl: Duration::from_secs(30),
            max_ttl: Duration::from_secs(300),
            attempt_timeout: Duration::from_secs(3),
        }
    }
}

/// Default upstream servers.
pub fn default_upstreams() -> Vec<IpAddr> {
    DEFAULT_UPSTREAMS
        .iter()
        .filter_map(|upstream| upstream.parse().ok())
        .collect()
}

/// Parse a comma separated list of upstream IPs.
///
/// Invalid entries are skipped; an empty or fully invalid list falls back to
/// [`default_upstreams`].
pub fn parse_upstreams(raw: &str) -> Vec<IpAddr> {
    let upstreams: Vec<IpAddr> = raw
        .split(',')
        .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
        .collect();

    if upstreams.is_empty() {
        default_upstreams()
    } else {
        upstreams
    }
}

/// A cached resolution, valid until `expires_at`.
#[derive(Debug, Clone, Copy)]
struct CacheEntry {
    ip: IpAddr,
    expires_at: Instant,
}

/// Hostname resolver with a TTL cache and multi-upstream fallback.
pub struct DnsResolver {
    upstreams: Vec<Arc<dyn Upstream>>,
    cache: Mutex<HashMap<String, CacheEntry>>,
    min_ttl: Duration,
    max_ttl: Duration,
    attempt_timeout: Duration,
}

impl DnsResolver {
    /// Build the production resolver: one `hickory`-backed upstream per
    /// configured server, tried in order.
    ///
    /// An upstream that cannot be built is logged and skipped, so the resolver
    /// always exists; if every upstream is unusable, resolution fails with
    /// [`DnsError::Timeout`] instead of panicking.
    pub fn new(config: DnsConfig) -> Self {
        let upstreams = config
            .upstreams
            .iter()
            .filter_map(
                |upstream| match HickoryUpstream::new(*upstream, config.attempt_timeout) {
                    Ok(upstream) => Some(Arc::new(upstream) as Arc<dyn Upstream>),
                    Err(error) => {
                        tracing::warn!(%upstream, %error, "DNS: upstream disabled");
                        None
                    }
                },
            )
            .collect();

        Self::with_upstreams(upstreams, config)
    }

    /// Build a resolver with injected upstreams (tests, alternative transports).
    ///
    /// `config.upstreams` is ignored: the given `upstreams` are used instead.
    pub fn with_upstreams(upstreams: Vec<Arc<dyn Upstream>>, config: DnsConfig) -> Self {
        Self {
            upstreams,
            cache: Mutex::new(HashMap::new()),
            min_ttl: config.min_ttl,
            // Guard against an inverted configuration: `Duration::clamp` panics
            // when the lower bound is greater than the upper one.
            max_ttl: config.max_ttl.max(config.min_ttl),
            attempt_timeout: config.attempt_timeout,
        }
    }

    /// Resolve `host` to an IP, serving it from the cache when possible.
    ///
    /// IP literals are returned as-is without querying any upstream.
    pub async fn resolve(&self, host: &str) -> Result<IpAddr, DnsError> {
        let host = normalize_host(host);

        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(ip);
        }

        if let Some(ip) = self.cached_ip(&host) {
            return Ok(ip);
        }

        self.query_upstreams(&host).await
    }

    /// Query every upstream in order until one answers.
    ///
    /// A timeout, a SERVFAIL or a transport error moves on to the next upstream;
    /// NXDOMAIN is authoritative and stops the fallback. When nothing answers,
    /// the most informative error is returned (a timeout only if every upstream
    /// timed out).
    async fn query_upstreams(&self, host: &str) -> Result<IpAddr, DnsError> {
        let mut first_non_timeout: Option<DnsError> = None;

        for upstream in &self.upstreams {
            match tokio::time::timeout(self.attempt_timeout, upstream.query(host)).await {
                Ok(Ok(answer)) => {
                    let ttl = answer.ttl.clamp(self.min_ttl, self.max_ttl);
                    self.store(host, answer.ip, ttl);
                    return Ok(answer.ip);
                }
                Ok(Err(DnsError::Nxdomain)) => return Err(DnsError::Nxdomain),
                Ok(Err(error)) => {
                    if !matches!(error, DnsError::Timeout) && first_non_timeout.is_none() {
                        first_non_timeout = Some(error);
                    }
                }
                // Attempt timeout: this upstream did not answer in time.
                Err(_elapsed) => {}
            }
        }

        Err(first_non_timeout.unwrap_or(DnsError::Timeout))
    }

    /// Cached IP for `host`, if the entry is still valid.
    fn cached_ip(&self, host: &str) -> Option<IpAddr> {
        let now = Instant::now();
        self.lock_cache()
            .get(host)
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.ip)
    }

    /// Store a resolution, keeping the cache within [`MAX_CACHE_ENTRIES`].
    ///
    /// Refreshing a known host never grows the map, so only new hosts can hit
    /// the bound: expired entries are dropped first, and if the cache is still
    /// full the soonest-to-expire entry makes room.
    fn store(&self, host: &str, ip: IpAddr, ttl: Duration) {
        let now = Instant::now();
        let mut cache = self.lock_cache();

        if cache.len() >= MAX_CACHE_ENTRIES && !cache.contains_key(host) {
            cache.retain(|_, entry| entry.expires_at > now);

            if cache.len() >= MAX_CACHE_ENTRIES {
                let soonest = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.expires_at)
                    .map(|(host, _)| host.clone());

                if let Some(soonest) = soonest {
                    cache.remove(&soonest);
                }
            }
        }

        cache.insert(
            host.to_string(),
            CacheEntry {
                ip,
                expires_at: now + ttl,
            },
        );
    }

    /// Poisoning is harmless here: the map is always left consistent.
    fn lock_cache(&self) -> MutexGuard<'_, HashMap<String, CacheEntry>> {
        self.cache.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Insert an entry directly, simulating a warm cache.
    #[cfg(test)]
    pub(crate) fn prime_cache(&self, host: &str, ip: IpAddr, ttl: Duration) {
        self.store(&normalize_host(host), ip, ttl);
    }

    #[cfg(test)]
    fn cached(&self, host: &str) -> Option<IpAddr> {
        self.cached_ip(&normalize_host(host))
    }

    #[cfg(test)]
    fn cache_len(&self) -> usize {
        self.lock_cache().len()
    }

    #[cfg(test)]
    fn cached_ttl(&self, host: &str) -> Option<Duration> {
        let now = Instant::now();
        self.lock_cache()
            .get(&normalize_host(host))
            .filter(|entry| entry.expires_at > now)
            .map(|entry| entry.expires_at - now)
    }
}

/// Upstream servers installed at startup, before the resolver is built.
static UPSTREAMS: OnceLock<Vec<IpAddr>> = OnceLock::new();

/// Install the upstream list coming from `Config`.
///
/// Called once at startup; if it never runs, the resolver falls back to
/// [`default_upstreams`]. Only the first call wins, so a late caller cannot
/// change the upstreams under a resolver that is already in use.
pub fn init(upstreams: Vec<IpAddr>) {
    if UPSTREAMS.set(upstreams).is_err() {
        tracing::warn!("DNS: upstreams already initialized, ignoring new list");
    }
}

/// Process-wide resolver, built on first use.
pub fn resolver() -> &'static Arc<DnsResolver> {
    static RESOLVER: OnceLock<Arc<DnsResolver>> = OnceLock::new();

    RESOLVER.get_or_init(|| {
        let mut config = DnsConfig::default();

        if let Some(upstreams) = UPSTREAMS.get() {
            config.upstreams = upstreams.clone();
        }

        tracing::debug!(
            upstreams = ?config.upstreams,
            min_ttl_secs = config.min_ttl.as_secs(),
            max_ttl_secs = config.max_ttl.as_secs(),
            "DNS: resolver initialized"
        );

        Arc::new(DnsResolver::new(config))
    })
}

/// Resolve `host` with the process-wide resolver.
pub async fn resolve(host: &str) -> Result<IpAddr, DnsError> {
    resolver().resolve(host).await
}

/// Lower-case, dot-less and bracket-less key for the cache.
fn normalize_host(host: &str) -> String {
    let host = host.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);

    host.trim_end_matches('.').to_ascii_lowercase()
}

/// Adapter that plugs the resolver into `reqwest`.
///
/// Injecting it through `ClientBuilder::dns_resolver` keeps `reqwest` convinced
/// that the destination is the hostname, so SNI and the `Host` header are
/// preserved.
pub struct ReqwestDnsResolver(Arc<DnsResolver>);

/// Resolver adapter for `reqwest::ClientBuilder::dns_resolver`.
pub fn reqwest_resolver() -> Arc<ReqwestDnsResolver> {
    Arc::new(ReqwestDnsResolver(Arc::clone(resolver())))
}

impl reqwest::dns::Resolve for ReqwestDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        let resolver = Arc::clone(&self.0);

        Box::pin(async move {
            let ip = resolver
                .resolve(&host)
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?;

            // Port 0 lets reqwest fill in the URL port (or the scheme default).
            let addrs: reqwest::dns::Addrs = Box::new(std::iter::once(SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}

/// A single `hickory` upstream.
struct HickoryUpstream {
    resolver: TokioResolver,
}

impl HickoryUpstream {
    /// Build a resolver talking to exactly one server.
    fn new(server: IpAddr, timeout: Duration) -> Result<Self, DnsError> {
        let mut options = ResolverOpts::default();
        options.timeout = timeout;
        options.attempts = 1;
        // The TTL cache lives in this module: disabling hickory's keeps a single
        // source of truth for `min_ttl` / `max_ttl`. No retries either.
        options.cache_size = 0;
        options.try_tcp_on_error = false;
        options.ip_strategy = LookupIpStrategy::Ipv4thenIpv6;

        let config = ResolverConfig::from_parts(
            None,
            Vec::new(),
            vec![NameServerConfig::udp_and_tcp(server)],
        );

        let resolver = Resolver::builder_with_config(config, TokioRuntimeProvider::default())
            .with_options(options)
            .build()
            .map_err(|error| {
                DnsError::Other(format!("cannot build resolver for {server}: {error}"))
            })?;

        Ok(Self { resolver })
    }
}

#[async_trait]
impl Upstream for HickoryUpstream {
    async fn query(&self, host: &str) -> Result<UpstreamAnswer, DnsError> {
        let mut name = Name::from_utf8(host)
            .map_err(|error| DnsError::Other(format!("invalid host {host}: {error}")))?;
        // Fully qualified, so no search domain gets involved.
        name.set_fqdn(true);

        let lookup = self
            .resolver
            .lookup_ip(name)
            .await
            .map_err(classify_hickory_error)?;

        let ip = lookup
            .iter()
            .next()
            .ok_or_else(|| DnsError::Other(format!("no address for {host}")))?;

        // The shortest TTL among the address records bounds the cache entry.
        let ttl_secs = lookup
            .as_lookup()
            .answers()
            .iter()
            .filter(|record| matches!(record.record_type(), RecordType::A | RecordType::AAAA))
            .map(|record| record.ttl)
            .min()
            .unwrap_or(0);

        Ok(UpstreamAnswer {
            ip,
            ttl: Duration::from_secs(u64::from(ttl_secs)),
        })
    }
}

/// Map a `hickory` failure onto the typed contract consumed by the checkers.
fn classify_hickory_error(error: NetError) -> DnsError {
    if error.is_nx_domain() {
        return DnsError::Nxdomain;
    }

    match &error {
        NetError::Timeout => DnsError::Timeout,
        NetError::Dns(HickoryDnsError::ResponseCode(ResponseCode::ServFail)) => DnsError::Servfail,
        NetError::Dns(HickoryDnsError::NoRecordsFound(records))
            if records.response_code == ResponseCode::ServFail =>
        {
            DnsError::Servfail
        }
        other => DnsError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    const TEST_HOST: &str = "watchbeat-cache.test";

    fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    /// Scripted upstream, so cache / fallback / error classification can be
    /// exercised without touching the network.
    struct FakeUpstream {
        behaviour: Behaviour,
        calls: AtomicUsize,
    }

    enum Behaviour {
        /// Answer with an IP and a TTL (in seconds).
        Answer(IpAddr, u64),
        /// Behave like an upstream that never answers.
        Hang(Duration),
        /// Fail with a scripted error.
        Fail(fn() -> DnsError),
    }

    impl FakeUpstream {
        fn answer(ip: IpAddr, ttl_secs: u64) -> Self {
            Self {
                behaviour: Behaviour::Answer(ip, ttl_secs),
                calls: AtomicUsize::new(0),
            }
        }

        fn hanging(how_long: Duration) -> Self {
            Self {
                behaviour: Behaviour::Hang(how_long),
                calls: AtomicUsize::new(0),
            }
        }

        fn failing(error: fn() -> DnsError) -> Self {
            Self {
                behaviour: Behaviour::Fail(error),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl Upstream for FakeUpstream {
        async fn query(&self, _host: &str) -> Result<UpstreamAnswer, DnsError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match &self.behaviour {
                Behaviour::Answer(ip, ttl_secs) => Ok(UpstreamAnswer {
                    ip: *ip,
                    ttl: Duration::from_secs(*ttl_secs),
                }),
                Behaviour::Hang(how_long) => {
                    tokio::time::sleep(*how_long).await;
                    Err(DnsError::Timeout)
                }
                Behaviour::Fail(error) => Err(error()),
            }
        }
    }

    /// Resolver wired to the given fake upstreams.
    fn resolver_with(upstreams: Vec<Arc<FakeUpstream>>, min_ttl: u64, max_ttl: u64) -> DnsResolver {
        let upstreams: Vec<Arc<dyn Upstream>> = upstreams
            .into_iter()
            .map(|upstream| upstream as Arc<dyn Upstream>)
            .collect();

        DnsResolver::with_upstreams(
            upstreams,
            DnsConfig {
                min_ttl: Duration::from_secs(min_ttl),
                max_ttl: Duration::from_secs(max_ttl),
                attempt_timeout: Duration::from_millis(100),
                ..DnsConfig::default()
            },
        )
    }

    // ───── Requirement: Resolución de nombres con caché TTL ─────

    /// Scenario: Resolución de un host nuevo.
    #[tokio::test]
    async fn resolve_should_query_upstream_and_cache_result_for_new_host() {
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 10), 60));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        let resolved = resolver.resolve(TEST_HOST).await;

        assert_eq!(resolved, Ok(ip(203, 0, 113, 10)));
        assert_eq!(upstream.calls(), 1, "upstream must be queried once");
        assert_eq!(
            resolver.cached(TEST_HOST),
            Some(ip(203, 0, 113, 10)),
            "resolved entry must be cached"
        );
    }

    /// Scenario: Segunda resolución dentro del TTL no consulta upstream.
    #[tokio::test]
    async fn resolve_should_serve_cached_ip_without_querying_upstream_within_ttl() {
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 11), 60));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        let first = resolver.resolve(TEST_HOST).await;
        let second = resolver.resolve(TEST_HOST).await;

        assert_eq!(first, Ok(ip(203, 0, 113, 11)));
        assert_eq!(second, Ok(ip(203, 0, 113, 11)));
        assert_eq!(
            upstream.calls(),
            1,
            "second lookup must come from the cache"
        );
    }

    /// Scenario: Entrada expirada se resuelve de nuevo.
    #[tokio::test]
    async fn resolve_should_query_upstream_again_when_entry_expired() {
        // TTL 1s is above the 0s lower bound, so the entry expires on its own.
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 12), 1));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        resolver.resolve(TEST_HOST).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        resolver.resolve(TEST_HOST).await.unwrap();

        assert_eq!(upstream.calls(), 2, "expired entry must be resolved again");
    }

    /// The cache is an upper-bounded structure, as documented.
    #[tokio::test]
    async fn cache_should_not_grow_beyond_upper_bound() {
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 60), 3600));
        let resolver = resolver_with(vec![upstream], 0, 3600);

        for index in 0..(MAX_CACHE_ENTRIES + 50) {
            resolver
                .resolve(&format!("host-{index}.test"))
                .await
                .unwrap();
        }

        assert!(
            resolver.cache_len() <= MAX_CACHE_ENTRIES,
            "cache holds {} entries, above the {MAX_CACHE_ENTRIES} upper bound",
            resolver.cache_len()
        );
    }

    /// D3: TTL is clamped between the configured lower and upper bounds.
    #[tokio::test]
    async fn cached_ttl_should_be_clamped_between_configured_bounds() {
        let short = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 13), 1));
        let clamped_low = resolver_with(vec![short], 30, 300);
        clamped_low.resolve(TEST_HOST).await.unwrap();
        let low_ttl = clamped_low.cached_ttl(TEST_HOST).unwrap();
        assert!(
            low_ttl > Duration::from_secs(29) && low_ttl <= Duration::from_secs(30),
            "short TTL must be raised to the 30s lower bound, got {low_ttl:?}"
        );

        let long = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 14), 3600));
        let clamped_high = resolver_with(vec![long], 30, 300);
        clamped_high.resolve(TEST_HOST).await.unwrap();
        let high_ttl = clamped_high.cached_ttl(TEST_HOST).unwrap();
        assert!(
            high_ttl <= Duration::from_secs(300) && high_ttl > Duration::from_secs(299),
            "long TTL must be capped at the 300s upper bound, got {high_ttl:?}"
        );
    }

    #[tokio::test]
    async fn cache_should_be_case_insensitive_and_ignore_trailing_dot() {
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 15), 60));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        resolver.resolve("WatchBeat-Cache.TEST.").await.unwrap();
        resolver.resolve(TEST_HOST).await.unwrap();

        assert_eq!(upstream.calls(), 1, "both spellings share one cache entry");
    }

    #[tokio::test]
    async fn resolve_should_return_ip_literal_without_querying_upstream() {
        let upstream = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 16), 60));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        let resolved = resolver.resolve("127.0.0.1").await;

        assert_eq!(resolved, Ok(ip(127, 0, 0, 1)));
        assert_eq!(upstream.calls(), 0, "IP literals need no DNS query");
    }

    // ───── Requirement: Fallback entre múltiples upstreams ─────

    /// Scenario: El primer upstream no responde.
    #[tokio::test]
    async fn resolve_should_fall_back_to_second_upstream_when_first_does_not_respond() {
        let dead = Arc::new(FakeUpstream::hanging(Duration::from_secs(5)));
        let alive = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 20), 60));
        let resolver = resolver_with(vec![Arc::clone(&dead), Arc::clone(&alive)], 0, 300);

        let started = std::time::Instant::now();
        let resolved = resolver.resolve(TEST_HOST).await;

        assert_eq!(resolved, Ok(ip(203, 0, 113, 20)));
        assert_eq!(dead.calls(), 1, "first upstream must be tried");
        assert_eq!(alive.calls(), 1, "second upstream must be used as fallback");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the attempt timeout must cut off the hanging upstream"
        );
    }

    /// Scenario: Todos los upstreams fallan.
    #[tokio::test]
    async fn resolve_should_return_timeout_when_no_upstream_responds() {
        let first = Arc::new(FakeUpstream::hanging(Duration::from_secs(5)));
        let second = Arc::new(FakeUpstream::hanging(Duration::from_secs(5)));
        let resolver = resolver_with(vec![Arc::clone(&first), Arc::clone(&second)], 0, 300);

        let resolved = resolver.resolve(TEST_HOST).await;

        assert_eq!(resolved, Err(DnsError::Timeout));
        assert_eq!(first.calls(), 1);
        assert_eq!(second.calls(), 1);
        assert_eq!(resolver.cached(TEST_HOST), None, "no false positive cached");
    }

    // ───── Requirement: Clasificación de errores de resolución ─────

    /// Scenario: Dominio inexistente.
    #[tokio::test]
    async fn resolve_should_return_nxdomain_error() {
        let upstream = Arc::new(FakeUpstream::failing(|| DnsError::Nxdomain));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        assert_eq!(resolver.resolve(TEST_HOST).await, Err(DnsError::Nxdomain));
        assert_eq!(resolver.cached(TEST_HOST), None);
    }

    /// Scenario: Fallo del servidor DNS.
    #[tokio::test]
    async fn resolve_should_return_servfail_error() {
        let upstream = Arc::new(FakeUpstream::failing(|| DnsError::Servfail));
        let resolver = resolver_with(vec![Arc::clone(&upstream)], 0, 300);

        assert_eq!(resolver.resolve(TEST_HOST).await, Err(DnsError::Servfail));
    }

    /// A SERVFAIL upstream is not authoritative: the next one is still tried.
    #[tokio::test]
    async fn resolve_should_fall_back_when_upstream_returns_servfail() {
        let servfail = Arc::new(FakeUpstream::failing(|| DnsError::Servfail));
        let alive = Arc::new(FakeUpstream::answer(ip(203, 0, 113, 30), 60));
        let resolver = resolver_with(vec![servfail, Arc::clone(&alive)], 0, 300);

        assert_eq!(resolver.resolve(TEST_HOST).await, Ok(ip(203, 0, 113, 30)));
        assert_eq!(alive.calls(), 1);
    }

    // ───── Upstream configuration ─────

    #[test]
    fn parse_upstreams_should_accept_comma_separated_ips() {
        assert_eq!(
            parse_upstreams("9.9.9.9, 1.0.0.1"),
            vec![ip(9, 9, 9, 9), ip(1, 0, 0, 1)]
        );
    }

    #[test]
    fn parse_upstreams_should_ignore_invalid_entries() {
        assert_eq!(parse_upstreams("not-an-ip,8.8.8.8"), vec![ip(8, 8, 8, 8)]);
    }

    #[test]
    fn parse_upstreams_should_fall_back_to_defaults_when_empty() {
        assert_eq!(parse_upstreams("   "), default_upstreams());
    }

    #[test]
    fn default_upstreams_should_be_the_documented_ones() {
        assert_eq!(
            default_upstreams(),
            vec![ip(1, 1, 1, 1), ip(8, 8, 8, 8), ip(9, 9, 9, 9)]
        );
    }

    // ───── Requirement: Integración transparente en los checkers ─────

    /// Scenario: Check HTTPS resuelto por la caché.
    ///
    /// The request target is a hostname that does not exist in the public DNS,
    /// so the only way the request can reach the local server is by using the
    /// IP warmed up in the process-wide resolver cache. The `Host` header must
    /// keep the hostname (same reason SNI does: reqwest still believes the
    /// destination is the hostname).
    #[tokio::test]
    async fn http_client_should_connect_to_cached_ip_keeping_host_header() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (head_tx, head_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1024];
            let read = stream.read(&mut buf).await.unwrap();
            let _ = head_tx.send(String::from_utf8_lossy(&buf[..read]).into_owned());
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        resolver().prime_cache(TEST_HOST, ip(127, 0, 0, 1), Duration::from_secs(60));

        let client = crate::checker::build_http_client(reqwest_resolver())
            .expect("client with the custom resolver must build");

        let response = client
            .get(format!("http://{TEST_HOST}:{port}/"))
            .send()
            .await
            .expect("request must reach the cached IP");

        assert_eq!(response.status(), 200);

        let head = head_rx.await.expect("server must report the request head");
        // hyper writes HTTP/1.1 header names in lower case.
        assert!(
            head.to_ascii_lowercase()
                .contains(&format!("host: {TEST_HOST}:{port}")),
            "Host header must keep the hostname, got: {head}"
        );
    }
}
