//! Storm breaker — anti-tormenta de alertas (Fase 5).
//!
//! Una incidencia de infraestructura (red, DNS, proveedor) tumba varios
//! monitores a la vez. Sin este módulo cada caída dispara su propia
//! notificación; con él, cuando el número de caídas dentro de una ventana de
//! `window_seconds` alcanza `max(min_monitors, ceil(X % · monitores_habilitados))`
//! se emite **una sola** alerta agregada y se suprimen las individuales hasta
//! que la tormenta se rearma (D1–D5 en `design.md`).
//!
//! La máquina de estados es **pura**: recibe `now: Instant` en cada llamada, así
//! los tests avanzan el reloj a mano y no necesitan `sleep` real.
//!
//! Desde #72 la misma lógica agrupa además las señales de `latency` y `expiry`,
//! cada tipo con su **episodio independiente** (ver [`SignalKind`] y
//! [`StormBreaker::on_signal`]).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::db::Database;

/// Setting key: master switch for the breaker.
pub const SETTING_ENABLED: &str = "storm_breaker_enabled";
/// Setting key: threshold as a percentage of the enabled monitors.
pub const SETTING_THRESHOLD_PERCENT: &str = "storm_threshold_percent";
/// Setting key: detection window, in seconds.
pub const SETTING_WINDOW_SECONDS: &str = "storm_window_seconds";
/// Setting key: absolute floor for the threshold.
pub const SETTING_MIN_MONITORS: &str = "storm_min_monitors";
/// Setting key: template used to render the aggregated alert.
pub const SETTING_TEMPLATE_STORM: &str = "default_template_storm";

/// Configuration of the storm breaker, loaded from the global settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakerConfig {
    /// Master switch. When `false` the breaker never suppresses nor aggregates.
    pub enabled: bool,
    /// Threshold `X`, as a percentage of the enabled monitors.
    pub threshold_percent: u8,
    /// Detection window `Y`, in seconds.
    pub window_seconds: u64,
    /// Absolute floor for the threshold: fewer enabled monitors than this can
    /// never declare a storm.
    pub min_monitors: usize,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percent: 50,
            window_seconds: 10,
            min_monitors: 3,
        }
    }
}

impl BreakerConfig {
    /// Load the configuration from the settings KV table, falling back to the
    /// defaults for every missing (or unparsable) key (D6).
    pub async fn load(db: &Database) -> Self {
        let defaults = Self::default();

        let enabled = read_setting(db, SETTING_ENABLED)
            .await
            .and_then(|raw| parse_bool(&raw))
            .unwrap_or(defaults.enabled);

        // `threshold_percent` is a percentage: values above 100 are clamped so the
        // threshold stays representable (D3).
        let threshold_percent = read_setting(db, SETTING_THRESHOLD_PERCENT)
            .await
            .and_then(|raw| parse_u64(&raw))
            .map(|n| n.min(100) as u8)
            .unwrap_or(defaults.threshold_percent);

        // A zero-second window could never accumulate events, so the value is
        // floored at 1 s.
        let window_seconds = read_setting(db, SETTING_WINDOW_SECONDS)
            .await
            .and_then(|raw| parse_u64(&raw))
            .map(|n| n.max(1))
            .unwrap_or(defaults.window_seconds);

        // A zero floor would let a single down declare a storm, so it is floored
        // at 1 (D3: the absolute floor is what keeps a lone down individual).
        let min_monitors = read_setting(db, SETTING_MIN_MONITORS)
            .await
            .and_then(|raw| parse_u64(&raw))
            .map(|n| n.max(1) as usize)
            .unwrap_or(defaults.min_monitors);

        Self {
            enabled,
            threshold_percent,
            window_seconds,
            min_monitors,
        }
    }
}

/// Read a setting, reporting (instead of swallowing) a DB failure (MENOR-3).
///
/// A failed read is still treated as "not configured" — the default wins — but it
/// is logged, so a broken settings table is not mistaken for an empty one.
async fn read_setting(db: &Database, key: &str) -> Option<String> {
    match db.get_setting(key).await {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!("Breaker: cannot read setting '{key}': {e}");
            None
        }
    }
}

/// Parse a boolean setting (`true`/`false`, plus the usual shorthands).
fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Parse an unsigned integer setting, ignoring any surrounding whitespace.
fn parse_u64(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

/// The breaker is either letting everything pass (`Closed`) or suppressing the
/// individual notifications of an ongoing storm (`Open`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
}

impl BreakerState {
    /// The state as it is stored/exposed (settings, logs, future API).
    pub fn as_str(&self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::Open => "open",
        }
    }
}

/// The aggregated alert emitted exactly once per storm episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateAlert {
    /// Number of **unique** monitors that triggered the episode inside the window:
    /// downs for the storm episode, `latency`/`expiry` signals for theirs (#72).
    pub count: usize,
    /// The affected monitors' ids, unique and in order of appearance.
    pub monitor_ids: Vec<String>,
    /// The affected monitors' names, same order as [`Self::monitor_ids`].
    pub monitors: Vec<String>,
    /// The window that declared the storm, in seconds.
    pub window_seconds: u64,
    /// The absolute threshold that was reached.
    pub threshold: usize,
}

/// What the breaker wants the scheduler to do with a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakerDecision {
    /// Dispatch the notification as usual.
    Pass,
    /// Drop the notification: a storm is already being reported.
    Suppress,
    /// A storm just started: dispatch this single aggregated alert instead.
    OpenAggregate(AggregateAlert),
}

/// The kind of signal the breaker can aggregate, besides downs (feature #72).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    /// A monitor breached its latency threshold.
    Latency,
    /// A monitor's TLS certificate is about to expire.
    Expiry,
}

impl SignalKind {
    /// Stable label, used by the scheduler to pick the aggregated template.
    pub fn as_str(self) -> &'static str {
        match self {
            SignalKind::Latency => "latency",
            SignalKind::Expiry => "expiry",
        }
    }

    /// Inverse of [`Self::as_str`]: the single source of truth for label ↔ kind.
    ///
    /// Returns `None` for the labels that are not signals (`down`, `up`, …).
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "latency" => Some(SignalKind::Latency),
            "expiry" => Some(SignalKind::Expiry),
            _ => None,
        }
    }
}

/// How an episode treats a new transition while it is already `Open`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhileOpen {
    /// Refresh the monitor's event (keeps the episode alive while it keeps falling).
    Refresh,
    /// Do not record it: the aggregated alert is already out and refreshing would
    /// only stretch the episode (M2).
    Ignore,
}

/// A transition observed by the breaker: when it happened and to whom.
type Event = (Instant, String, String);

/// One aggregation episode: the state plus the transitions still inside the
/// window. The breaker keeps one per kind (`down`, `latency`, `expiry`), so the
/// kinds never interfere with each other (feature #72).
struct Episode {
    state: BreakerState,
    /// Transitions inside the window, unique per monitor. *Not* sorted by instant:
    /// an in-place refresh can leave a stale entry behind a fresher one.
    events: VecDeque<Event>,
}

impl Episode {
    /// A closed episode with no observed transitions.
    fn new() -> Self {
        Self {
            state: BreakerState::Closed,
            events: VecDeque::new(),
        }
    }

    /// Rearm the episode and forget its transitions (lazy rearm or shutdown).
    fn clear(&mut self) {
        self.state = BreakerState::Closed;
        self.events.clear();
    }

    /// Record a transition, keyed by monitor.
    ///
    /// The threshold counts **monitors**, not transitions (MAYOR-1): when the same
    /// monitor is already inside the window its event is refreshed in place (new
    /// instant and name) instead of being pushed again, so one flapping monitor can
    /// never declare a storm on its own and silence the rest of the fleet.
    fn record(&mut self, now: Instant, monitor_id: &str, monitor_name: &str) {
        if let Some(event) = self
            .events
            .iter_mut()
            .find(|(_, id, _)| id.as_str() == monitor_id)
        {
            *event = (now, monitor_id.to_string(), monitor_name.to_string());
            return;
        }

        self.events
            .push_back((now, monitor_id.to_string(), monitor_name.to_string()));
    }

    /// Drop the transitions that fell out of the window.
    ///
    /// An event exactly `window` old is already *out*: the boundary is closed,
    /// which is the same criterion [`Self::maybe_close`] uses to rearm (NIT-2).
    ///
    /// `retain` instead of popping from the front: [`Self::record`] refreshes
    /// events **in place**, so the deque is no longer sorted by instant and a stale
    /// entry can sit behind a fresher one (REGRESIÓN MAYOR-1).
    fn prune(&mut self, now: Instant, window: Duration) {
        self.events
            .retain(|(at, _, _)| now.saturating_duration_since(*at) < window);
    }

    /// Lazily rearm the episode once the window elapsed without new transitions.
    fn maybe_close(&mut self, now: Instant, window: Duration) {
        self.prune(now, window);
        if self.state != BreakerState::Open {
            return;
        }

        // The latest transition is the maximum instant, not `back()`: in-place
        // refreshes mean the deque is not sorted (REGRESIÓN MAYOR-1).
        let rearmed = match self.events.iter().map(|(at, _, _)| *at).max() {
            Some(last) => now.saturating_duration_since(last) >= window,
            // No event left inside the window: the episode is over.
            None => true,
        };

        if rearmed {
            self.clear();
        }
    }

    /// Snapshot of the current episode as an aggregated alert.
    ///
    /// `events` already holds one entry per unique monitor, so both the count and
    /// the lists describe *distinct* monitors (MAYOR-1).
    fn aggregate(&self, threshold: usize, window_seconds: u64) -> AggregateAlert {
        let mut monitor_ids: Vec<String> = Vec::new();
        let mut monitors: Vec<String> = Vec::new();
        for (_, id, name) in &self.events {
            if !monitor_ids.iter().any(|known| known == id) {
                monitor_ids.push(id.clone());
                monitors.push(name.clone());
            }
        }

        AggregateAlert {
            count: self.events.len(),
            monitor_ids,
            monitors,
            window_seconds,
            threshold,
        }
    }
}

/// Absolute threshold for the given number of enabled monitors:
/// `max(min_monitors, ceil(threshold_percent · enabled / 100))`, never zero.
fn threshold(cfg: &BreakerConfig, enabled_monitors: usize) -> usize {
    let by_percent = (enabled_monitors * cfg.threshold_percent as usize).div_ceil(100);
    // `.max(1)` guards a hand-built `min_monitors = 0`: a zero threshold would let
    // the very first transition declare a storm (n6).
    by_percent.max(cfg.min_monitors).max(1)
}

/// Pure, clock-injected state machine detecting alert storms.
///
/// It holds one independent episode per kind: downs (`on_down`/`on_up`) plus the
/// `latency`/`expiry` signals (`on_signal`, feature #72). Opening or suppressing
/// one kind never affects the state nor the dispatch of the others.
pub struct StormBreaker {
    cfg: BreakerConfig,
    /// The original (and visible) episode: monitor downs and recoveries.
    down: Episode,
    /// Latency breaches.
    latency: Episode,
    /// Certificate expirations.
    expiry: Episode,
}

impl StormBreaker {
    /// Build a breaker in the `Closed` state with the given configuration.
    pub fn new(cfg: BreakerConfig) -> Self {
        Self {
            cfg,
            down: Episode::new(),
            latency: Episode::new(),
            expiry: Episode::new(),
        }
    }

    /// Apply a new configuration in place (hot reload). Disabling the breaker
    /// closes every episode and forgets it.
    pub fn configure(&mut self, cfg: BreakerConfig) {
        self.cfg = cfg;
        if !cfg.enabled {
            self.down.clear();
            self.latency.clear();
            self.expiry.clear();
        }
    }

    /// The configuration currently in use.
    pub fn config(&self) -> BreakerConfig {
        self.cfg
    }

    /// The state of the **downs** episode (the original, visible one).
    pub fn state(&self) -> BreakerState {
        self.down.state
    }

    /// Report a monitor going down and get the decision for this transition.
    pub fn on_down(
        &mut self,
        now: Instant,
        monitor_id: &str,
        monitor_name: &str,
        enabled_monitors: usize,
    ) -> BreakerDecision {
        let cfg = self.cfg;
        // A down inside an open storm keeps refreshing the episode (MAYOR-1): the
        // fleet is still falling, so the storm is still on (see the regression test
        // `refreshing_a_non_final_event_does_not_rearm_early`).
        Self::decide(
            &cfg,
            &mut self.down,
            now,
            monitor_id,
            monitor_name,
            enabled_monitors,
            WhileOpen::Refresh,
        )
    }

    /// Report a monitor recovering and get the decision for this transition.
    pub fn on_up(&mut self, now: Instant) -> BreakerDecision {
        if !self.cfg.enabled {
            return BreakerDecision::Pass;
        }

        self.down.maybe_close(now, self.window());
        if self.down.state == BreakerState::Open {
            BreakerDecision::Suppress
        } else {
            BreakerDecision::Pass
        }
    }

    /// Report a `latency`/`expiry` signal and get the decision for it (feature #72).
    ///
    /// Shares the window and the threshold with [`Self::on_down`], but counts the
    /// transitions of the signal's **own** episode: a latency storm neither opens
    /// nor suppresses the expiry or the downs episodes, and vice versa.
    pub fn on_signal(
        &mut self,
        now: Instant,
        kind: SignalKind,
        monitor_id: &str,
        monitor_name: &str,
        enabled_monitors: usize,
    ) -> BreakerDecision {
        let cfg = self.cfg;
        let episode = match kind {
            SignalKind::Latency => &mut self.latency,
            SignalKind::Expiry => &mut self.expiry,
        };
        // M2: a signal episode is NOT refreshed while `Open`. A monitor whose
        // `interval_seconds < window_seconds` re-emits the signal on every check;
        // refreshing its event would keep the last instant fresh forever, the
        // episode would never rearm and every signal of that kind would stay
        // silenced. After `window` without recorded signals the episode rearms.
        Self::decide(
            &cfg,
            episode,
            now,
            monitor_id,
            monitor_name,
            enabled_monitors,
            WhileOpen::Ignore,
        )
    }

    /// The detection window as a duration.
    fn window(&self) -> Duration {
        Duration::from_secs(self.cfg.window_seconds)
    }

    /// The episode machine shared by downs and signals (D1–D5, M2).
    ///
    /// Disabled → everything passes; lazy rearm → a new episode after the window;
    /// open → suppress what follows (see [`WhileOpen`] for whether the transition
    /// still refreshes the episode); record → one entry per unique monitor; floor →
    /// too few enabled monitors can never declare a storm; threshold → a single
    /// aggregated alert.
    fn decide(
        cfg: &BreakerConfig,
        episode: &mut Episode,
        now: Instant,
        monitor_id: &str,
        monitor_name: &str,
        enabled_monitors: usize,
        while_open: WhileOpen,
    ) -> BreakerDecision {
        // Disabled: classic behaviour, nothing is aggregated nor suppressed.
        if !cfg.enabled {
            return BreakerDecision::Pass;
        }

        // Lazy rearm (D2): a storm that saw no new event for a whole window is over
        // by the time of this call.
        episode.maybe_close(now, Duration::from_secs(cfg.window_seconds));

        // Storm in progress: the aggregated alert is already out (D4).
        if episode.state == BreakerState::Open {
            if while_open == WhileOpen::Refresh {
                episode.record(now, monitor_id, monitor_name);
            }
            return BreakerDecision::Suppress;
        }

        episode.record(now, monitor_id, monitor_name);

        // Too few enabled monitors to ever declare a storm (D3).
        if enabled_monitors < cfg.min_monitors {
            return BreakerDecision::Pass;
        }

        // The threshold counts monitors, not transitions (MAYOR-1): `events` holds
        // one entry per unique monitor inside the window.
        let threshold = threshold(cfg, enabled_monitors);
        if episode.events.len() >= threshold {
            episode.state = BreakerState::Open;
            BreakerDecision::OpenAggregate(episode.aggregate(threshold, cfg.window_seconds))
        } else {
            BreakerDecision::Pass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const T: Duration = Duration::from_secs(1);
    const MS: Duration = Duration::from_millis(1);

    fn base_cfg() -> BreakerConfig {
        BreakerConfig {
            enabled: true,
            threshold_percent: 50,
            window_seconds: 10,
            min_monitors: 3,
        }
    }

    fn aggregate(
        ids: &[&str],
        monitors: &[&str],
        window_seconds: u64,
        threshold: usize,
    ) -> BreakerDecision {
        assert_eq!(ids.len(), monitors.len(), "ids and names pair up");
        BreakerDecision::OpenAggregate(AggregateAlert {
            count: ids.len(),
            monitor_ids: ids.iter().map(|id| (*id).to_string()).collect(),
            monitors: monitors.iter().map(|name| (*name).to_string()).collect(),
            window_seconds,
            threshold,
        })
    }

    // ── Scenario: muchos monitores caen a la vez → una sola alerta ──

    #[test]
    fn many_monitors_down_within_window_emit_one_aggregate() {
        // 8 enabled monitors at 50 % → threshold ceil(4) = 4.
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 2 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        // The 4th down reaches the threshold: one aggregated alert, and only one.
        assert_eq!(
            breaker.on_down(t0 + 3 * T, "m4", "cache", 8),
            aggregate(
                &["m1", "m2", "m3", "m4"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
        assert_eq!(breaker.state(), BreakerState::Open);
    }

    #[test]
    fn aggregate_includes_count_and_monitor_list() {
        // 3 enabled monitors at 100 % → threshold 3.
        let cfg = BreakerConfig {
            threshold_percent: 100,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        breaker.on_down(t0, "m1", "web", 3);
        breaker.on_down(t0 + T, "m2", "api", 3);
        assert_eq!(
            breaker.on_down(t0 + 2 * T, "m3", "db", 3),
            aggregate(&["m1", "m2", "m3"], &["web", "api", "db"], 10, 3)
        );
    }

    // ── Scenario: pocas caídas aisladas → notificación individual ──

    #[test]
    fn few_isolated_downs_pass_individually() {
        // 8 enabled monitors at 50 % → threshold 4.
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 2 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    #[test]
    fn downs_spread_beyond_the_window_never_accumulate() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        // One down every 11 s: always outside the 10 s window.
        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + 11 * T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 22 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    // ── Scenario: umbral absoluto mínimo ──

    #[test]
    fn min_monitors_floor_keeps_single_monitor_individual() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(breaker.on_down(t0, "m1", "web", 1), BreakerDecision::Pass);
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    #[test]
    fn a_lone_down_never_declares_a_storm() {
        // 100 % of 1000 monitors would still be 1000; the floor is what matters here.
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(
            breaker.on_down(t0, "m1", "web", 1000),
            BreakerDecision::Pass
        );
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    // ── MAYOR-1: el umbral cuenta monitores únicos, no transiciones ──

    #[test]
    fn one_flapping_monitor_never_declares_a_storm() {
        // 8 enabled monitors at 50 % → threshold 4. A single monitor flapping
        // (down/up four times) is still *one* monitor falling, so it must never
        // open a storm and silence the rest of the fleet (MAYOR-1).
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for i in 0..8u32 {
            assert_eq!(
                breaker.on_down(t0 + i * T, "m1", "web", 8),
                BreakerDecision::Pass
            );
            assert_eq!(breaker.on_up(t0 + i * T), BreakerDecision::Pass);
        }

        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    #[test]
    fn distinct_monitors_are_counted_apart_from_a_flapping_one() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        // m1 flaps four times, all inside the window: still one unique monitor.
        for i in 0..4u32 {
            breaker.on_down(t0 + i * T, "m1", "web", 8);
            breaker.on_up(t0 + i * T);
        }

        // Three more *distinct* monitors fall → four unique monitors → storm.
        assert_eq!(
            breaker.on_down(t0 + 4 * T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 5 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 6 * T, "m4", "cache", 8),
            aggregate(
                &["m1", "m2", "m3", "m4"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    // ── REGRESIÓN MAYOR-1: el refresco in-place rompe el orden temporal ──

    /// Refrescar un monitor que **no** está al final de `events` no debe adelantar
    /// el rearme: la última caída real es el instante máximo, no el `back()`.
    #[test]
    fn refreshing_a_non_final_event_does_not_rearm_early() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8);
        }
        assert_eq!(breaker.state(), BreakerState::Open);

        // m0 is the *oldest* entry; refreshing it at 9.9 s keeps the episode alive.
        assert_eq!(
            breaker.on_down(t0 + 9_900 * MS, "m0", "web", 8),
            BreakerDecision::Suppress
        );

        // At 13.1 s the latest real down is m0@9.9 (3.2 s ago), so the breaker must
        // NOT rearm. With the old `back()` it closed on the stale m3@3 (10.1 s ago).
        assert_eq!(breaker.on_up(t0 + 13_100 * MS), BreakerDecision::Suppress);
        assert_eq!(breaker.state(), BreakerState::Open);

        // The same episode must not alert a second time.
        assert_eq!(
            breaker.on_down(t0 + 13_200 * MS, "m4", "worker", 8),
            BreakerDecision::Suppress
        );
        assert_eq!(breaker.state(), BreakerState::Open);
    }

    /// Una entrada obsoleta que queda **detrás** de un evento refrescado debe salir
    /// en `prune`: si sobrevive, cuenta como monitor caído y provoca un falso
    /// positivo de tormenta.
    #[test]
    fn a_stale_event_behind_a_refreshed_one_is_pruned() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );

        // m1 (the oldest) is refreshed at 5 s; m2@1 stays *behind* it.
        assert_eq!(
            breaker.on_down(t0 + 5_000 * MS, "m1", "web", 8),
            BreakerDecision::Pass
        );

        // At 11.5 s, m2@1 is 10.5 s old → out of the window, it must be dropped.
        assert_eq!(
            breaker.on_down(t0 + 11_500 * MS, "m3", "db", 8),
            BreakerDecision::Pass
        );

        // At 12 s only m1 (refreshed at 5 s) and m3 are left: with m4 that makes
        // three unique monitors, below the threshold of four. The stale m2 was the
        // fourth and used to declare a storm counting a monitor already recovered.
        assert_eq!(
            breaker.on_down(t0 + 12_000 * MS, "m4", "cache", 8),
            BreakerDecision::Pass
        );
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    // ── Scenario: dedupe por episodio ──

    #[test]
    fn a_fifth_down_within_the_episode_is_suppressed() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8);
        }
        assert_eq!(breaker.state(), BreakerState::Open);

        // Still inside the window: no second aggregate.
        assert_eq!(
            breaker.on_down(t0 + 4 * T, "m5", "worker", 8),
            BreakerDecision::Suppress
        );
        assert_eq!(breaker.state(), BreakerState::Open);
    }

    // ── Scenario: un episodio nuevo vuelve a alertar ──

    #[test]
    fn a_new_episode_alerts_again_after_rearm() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8);
        }
        assert_eq!(breaker.state(), BreakerState::Open);

        // 20 s after the last down: the window elapsed, the next down re-arms and
        // starts a brand new episode.
        let t1 = t0 + 20 * T;
        assert_eq!(breaker.on_down(t1, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t1 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t1 + 2 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t1 + 3 * T, "m4", "cache", 8),
            aggregate(
                &["m1", "m2", "m3", "m4"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    // ── Scenario: supresión de `up` durante la tormenta ──

    #[test]
    fn recoveries_are_suppressed_during_a_storm_and_pass_after_rearm() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8);
        }
        assert_eq!(breaker.state(), BreakerState::Open);

        // Mid-storm recovery: the individual notification is dropped.
        assert_eq!(breaker.on_up(t0 + 5 * T), BreakerDecision::Suppress);

        // Past the window: the breaker re-arms and the recovery notifies again.
        assert_eq!(breaker.on_up(t0 + 20 * T), BreakerDecision::Pass);
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    // ── Scenario: desactivado se comporta como antes ──

    #[test]
    fn disabled_breaker_always_passes() {
        let cfg = BreakerConfig {
            enabled: false,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache", "worker"].iter().enumerate() {
            assert_eq!(
                breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8),
                BreakerDecision::Pass
            );
        }
        assert_eq!(breaker.on_up(t0 + 6 * T), BreakerDecision::Pass);
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    #[test]
    fn disabling_mid_storm_closes_and_clears_the_episode() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();
        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_down(t0 + i as u32 * T, &format!("m{i}"), name, 8);
        }
        assert_eq!(breaker.state(), BreakerState::Open);

        breaker.configure(BreakerConfig {
            enabled: false,
            ..base_cfg()
        });

        assert_eq!(breaker.state(), BreakerState::Closed);
        assert_eq!(
            breaker.on_down(t0 + 5 * T, "m5", "worker", 8),
            BreakerDecision::Pass
        );
    }

    // ── Scenario: umbral del 100 % ──

    #[test]
    fn one_hundred_percent_threshold_requires_every_monitor() {
        let cfg = BreakerConfig {
            threshold_percent: 100,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 2 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    // ── Scenario: reconfiguración en caliente ──

    #[test]
    fn configure_applies_a_new_threshold_without_restarting() {
        // 8 monitors at 80 % → threshold 7: three downs are not a storm.
        let mut breaker = StormBreaker::new(BreakerConfig {
            threshold_percent: 80,
            ..base_cfg()
        });
        let t0 = Instant::now();
        assert_eq!(breaker.on_down(t0, "m1", "web", 8), BreakerDecision::Pass);
        assert_eq!(
            breaker.on_down(t0 + T, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 2 * T, "m3", "db", 8),
            BreakerDecision::Pass
        );

        // Hot reload down to 50 % → threshold 4: the next down opens the storm.
        breaker.configure(base_cfg());
        assert_eq!(breaker.config().threshold_percent, 50);
        assert_eq!(
            breaker.on_down(t0 + 3 * T, "m4", "cache", 8),
            aggregate(
                &["m1", "m2", "m3", "m4"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    // ── Scenario: detección reproducible con reloj inyectado ──

    #[test]
    fn decisions_are_deterministic_with_an_injected_clock() {
        // Same (offset, id, name) script, same decisions and final state.
        let script: [(Duration, &str, &str); 6] = [
            (T, "m1", "web"),
            (2 * T, "m2", "api"),
            (3 * T, "m3", "db"),
            (4 * T, "m4", "cache"),
            (5 * T, "m4", "cache"),
            (6 * T, "m5", "worker"),
        ];

        let run = || {
            let mut breaker = StormBreaker::new(base_cfg());
            let t0 = Instant::now();
            let decisions: Vec<BreakerDecision> = script
                .iter()
                .map(|(offset, id, name)| breaker.on_down(t0 + *offset, id, name, 8))
                .collect();
            (decisions, breaker.state())
        };

        let (first, first_state) = run();
        let (second, second_state) = run();

        assert_eq!(first, second);
        assert_eq!(first_state, second_state);
        assert_eq!(first_state, BreakerState::Open);
    }

    #[test]
    fn state_as_str_exposes_the_documented_names() {
        assert_eq!(BreakerState::Closed.as_str(), "closed");
        assert_eq!(BreakerState::Open.as_str(), "open");
    }

    // ── Configuración desde settings ──

    async fn setup_db() -> (Database, TempDir) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("breaker.db");
        let db = Database::open(&path).await.expect("db");
        (db, dir)
    }

    #[tokio::test]
    async fn load_falls_back_to_defaults_when_settings_are_missing() {
        let (db, _dir) = setup_db().await;
        assert_eq!(BreakerConfig::load(&db).await, BreakerConfig::default());
    }

    #[tokio::test]
    async fn load_reads_the_settings_kv() {
        let (db, _dir) = setup_db().await;
        db.set_setting(SETTING_ENABLED, "false").await.unwrap();
        db.set_setting(SETTING_THRESHOLD_PERCENT, "80")
            .await
            .unwrap();
        db.set_setting(SETTING_WINDOW_SECONDS, "30").await.unwrap();
        db.set_setting(SETTING_MIN_MONITORS, "5").await.unwrap();

        assert_eq!(
            BreakerConfig::load(&db).await,
            BreakerConfig {
                enabled: false,
                threshold_percent: 80,
                window_seconds: 30,
                min_monitors: 5,
            }
        );
    }

    #[tokio::test]
    async fn load_ignores_unparsable_values_and_sanitises_edges() {
        let (db, _dir) = setup_db().await;
        db.set_setting(SETTING_ENABLED, "maybe").await.unwrap();
        db.set_setting(SETTING_THRESHOLD_PERCENT, "900")
            .await
            .unwrap();
        db.set_setting(SETTING_WINDOW_SECONDS, "0").await.unwrap();
        db.set_setting(SETTING_MIN_MONITORS, "0").await.unwrap();

        let cfg = BreakerConfig::load(&db).await;
        assert!(cfg.enabled, "unparsable switch keeps the default");
        assert_eq!(cfg.threshold_percent, 100, "percent is clamped to 100");
        assert_eq!(cfg.window_seconds, 1, "a zero window is floored at 1 s");
        assert_eq!(cfg.min_monitors, 1, "a zero floor is floored at 1");
    }

    // ── Feature #72: agrupación de señales `latency`/`expiry` ──

    /// Ráfaga de latencia de 4 monitores distintos (8 habilitados, 50 % → umbral
    /// 4): una única agregada y las señales siguientes suprimidas (D-72-1).
    #[test]
    fn a_latency_burst_is_coalesced_into_one_aggregate() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db"].iter().enumerate() {
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T,
                    SignalKind::Latency,
                    &format!("m{i}"),
                    name,
                    8
                ),
                BreakerDecision::Pass
            );
        }

        // El cuarto monitor distinto cruza el umbral: una única alerta agregada.
        assert_eq!(
            breaker.on_signal(t0 + 3 * T, SignalKind::Latency, "m3", "cache", 8),
            aggregate(
                &["m0", "m1", "m2", "m3"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    /// Episodio abierto: las señales del MISMO tipo se suprimen (no hay segunda
    /// agregada) hasta el rearme.
    #[test]
    fn later_latency_signals_inside_the_episode_are_suppressed() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_signal(
                t0 + i as u32 * T,
                SignalKind::Latency,
                &format!("m{i}"),
                name,
                8,
            );
        }

        assert_eq!(
            breaker.on_signal(t0 + 4 * T, SignalKind::Latency, "m4", "worker", 8),
            BreakerDecision::Suppress
        );
        assert_eq!(
            breaker.on_signal(t0 + 5 * T, SignalKind::Latency, "m5", "mail", 8),
            BreakerDecision::Suppress
        );
    }

    /// Rearme perezoso: pasada la ventana sin nuevas señales de latencia, un nuevo
    /// episodio vuelve a agrupar (D-72-2).
    #[test]
    fn a_latency_burst_after_the_window_alerts_again() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_signal(
                t0 + i as u32 * T,
                SignalKind::Latency,
                &format!("m{i}"),
                name,
                8,
            );
        }
        assert_eq!(
            breaker.on_signal(t0 + 4 * T, SignalKind::Latency, "m4", "worker", 8),
            BreakerDecision::Suppress
        );

        // 20 s después del último aviso (t0+4): la ventana expiró y el episodio rearma.
        let t1 = t0 + 20 * T;
        assert_eq!(
            breaker.on_signal(t1, SignalKind::Latency, "m0", "web", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t1 + T, SignalKind::Latency, "m1", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t1 + 2 * T, SignalKind::Latency, "m2", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t1 + 3 * T, SignalKind::Latency, "m3", "cache", 8),
            aggregate(
                &["m0", "m1", "m2", "m3"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    /// Un único monitor que emite latencia repetidamente es UN monitor distinto: no
    /// declara tormenta y todas sus señales pasan (MAYOR-1 compartido).
    #[test]
    fn a_single_oscillating_monitor_never_declares_a_signal_storm() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for i in 0..8u32 {
            assert_eq!(
                breaker.on_signal(t0 + i * T, SignalKind::Latency, "m1", "web", 8),
                BreakerDecision::Pass
            );
        }
    }

    /// Los monitores distintos se cuentan aparte del que oscila: m1 tres veces sigue
    /// siendo uno, y con m2/m3/m4 son cuatro únicos → tormenta.
    #[test]
    fn distinct_monitors_are_counted_apart_from_a_flapping_latency_one() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for i in 0..3u32 {
            breaker.on_signal(t0 + i * T, SignalKind::Latency, "m1", "web", 8);
        }
        assert_eq!(
            breaker.on_signal(t0 + 3 * T, SignalKind::Latency, "m2", "api", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + 4 * T, SignalKind::Latency, "m3", "db", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + 5 * T, SignalKind::Latency, "m4", "cache", 8),
            aggregate(
                &["m1", "m2", "m3", "m4"],
                &["web", "api", "db", "cache"],
                10,
                4
            )
        );
    }

    /// Independencia por tipo (D-72-3): abrir la tormenta de latencia no toca el
    /// episodio de expiración ni el de caídas.
    #[test]
    fn a_latency_storm_does_not_suppress_expiry_or_down() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_signal(
                t0 + i as u32 * T,
                SignalKind::Latency,
                &format!("m{i}"),
                name,
                8,
            );
        }

        // La tormenta de latencia no abre el episodio de caídas.
        assert_eq!(breaker.state(), BreakerState::Closed);

        // La expiración mantiene su propio episodio: pasa hasta su umbral.
        assert_eq!(
            breaker.on_signal(t0 + 4 * T, SignalKind::Expiry, "e0", "tls-a", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + 5 * T, SignalKind::Expiry, "e1", "tls-b", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + 6 * T, SignalKind::Expiry, "e2", "tls-c", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + 7 * T, SignalKind::Expiry, "e3", "tls-d", 8),
            aggregate(
                &["e0", "e1", "e2", "e3"],
                &["tls-a", "tls-b", "tls-c", "tls-d"],
                10,
                4
            )
        );

        // Y una caída se evalúa con su propio episodio: pasa hasta su umbral.
        assert_eq!(
            breaker.on_down(t0 + 8 * T, "d0", "svc-a", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 9 * T, "d1", "svc-b", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_down(t0 + 10 * T, "d2", "svc-c", 8),
            BreakerDecision::Pass
        );

        // m4: el episodio de caídas sigue pudiendo abrirse con su propio umbral.
        assert_eq!(
            breaker.on_down(t0 + 11 * T, "d3", "svc-d", 8),
            aggregate(
                &["d0", "d1", "d2", "d3"],
                &["svc-a", "svc-b", "svc-c", "svc-d"],
                10,
                4
            )
        );
    }

    // ── M2: un monitor rápido no puede congelar el episodio de señal ──

    /// Un monitor con `interval_seconds < window_seconds` que emite la señal en cada
    /// check refrescaba su evento indefinidamente: el episodio nunca rearmaba y todas
    /// las señales del tipo quedaban suprimidas para siempre. Mientras `Open` la
    /// señal se descarta **sin** registrar, así que la ventana expira y el episodio
    /// rearma.
    #[test]
    fn a_fast_monitor_cannot_keep_a_signal_episode_open_forever() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        // Cuatro monitores distintos abren el episodio de latencia en t0..t0+3.
        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            breaker.on_signal(
                t0 + i as u32 * T,
                SignalKind::Latency,
                &format!("m{i}"),
                name,
                8,
            );
        }

        // m9 emite cada segundo (muy por debajo de la ventana de 10 s): se suprime
        // pero NO refresca el episodio.
        for i in 4..13u32 {
            assert_eq!(
                breaker.on_signal(t0 + i * T, SignalKind::Latency, "m9", "fast", 8),
                BreakerDecision::Suppress
            );
        }

        // A los 13 s la última señal registrada sigue siendo la que abrió el episodio
        // (t0+3): la ventana expiró, el episodio rearmó y esta señal pasa individual.
        assert_eq!(
            breaker.on_signal(t0 + 13 * T, SignalKind::Latency, "m9", "fast", 8),
            BreakerDecision::Pass
        );
    }

    // ── Límites del episodio (m3) ──

    /// Una señal con EXACTAMENTE `window_seconds` de antigüedad ya está fuera: el
    /// mismo criterio cerrado que usa el rearme (NIT-2), ahora en el episodio de señal.
    #[test]
    fn a_signal_exactly_one_window_old_is_already_out() {
        // 3 habilitados al 100 % → umbral 3.
        let cfg = BreakerConfig {
            threshold_percent: 100,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        assert_eq!(
            breaker.on_signal(t0, SignalKind::Expiry, "m1", "tls-a", 3),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + T, SignalKind::Expiry, "m2", "tls-b", 3),
            BreakerDecision::Pass
        );
        // m1@0 tiene exactamente 10 s = ventana: debe podarse, así que esta tercera
        // señal sólo suma dos monitores y no declara tormenta.
        assert_eq!(
            breaker.on_signal(t0 + 10 * T, SignalKind::Expiry, "m3", "tls-c", 3),
            BreakerDecision::Pass
        );
    }

    /// El suelo `min_monitors` domina al porcentaje: con 4 habilitados y 50 %,
    /// `ceil(2)` no basta, hacen falta 4.
    #[test]
    fn the_min_monitors_floor_wins_over_the_percentage() {
        let cfg = BreakerConfig {
            threshold_percent: 50,
            min_monitors: 4,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        for (i, name) in ["tls-a", "tls-b", "tls-c"].iter().enumerate() {
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T,
                    SignalKind::Expiry,
                    &format!("m{i}"),
                    name,
                    4
                ),
                BreakerDecision::Pass
            );
        }
        assert_eq!(
            breaker.on_signal(t0 + 3 * T, SignalKind::Expiry, "m3", "tls-d", 4),
            aggregate(
                &["m0", "m1", "m2", "m3"],
                &["tls-a", "tls-b", "tls-c", "tls-d"],
                10,
                4
            )
        );
    }

    /// Los mismos monitores emitiendo dos tipos distintos, cada uno por debajo del
    /// umbral, producen dos flujos `Pass` independientes: las señales no se suman
    /// entre tipos.
    #[test]
    fn the_same_monitors_emitting_two_kinds_below_threshold_pass_separately() {
        // 8 habilitados al 50 % → umbral 4 por tipo.
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db"].iter().enumerate() {
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T,
                    SignalKind::Latency,
                    &format!("m{i}"),
                    name,
                    8
                ),
                BreakerDecision::Pass
            );
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T + MS,
                    SignalKind::Expiry,
                    &format!("m{i}"),
                    name,
                    8
                ),
                BreakerDecision::Pass
            );
        }
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    /// `from_label` es la única fuente de verdad etiqueta ↔ tipo (m5).
    #[test]
    fn signal_kind_round_trips_with_its_label() {
        for kind in [SignalKind::Latency, SignalKind::Expiry] {
            assert_eq!(SignalKind::from_label(kind.as_str()), Some(kind));
        }
        assert_eq!(SignalKind::from_label("down"), None);
        assert_eq!(SignalKind::from_label(""), None);
    }

    /// Un monitor único (menos habilitados que el suelo `min_monitors`) nunca
    /// declara tormenta de señal (misma regla que las caídas).
    #[test]
    fn fewer_monitors_than_the_floor_never_declare_a_signal_storm() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache"].iter().enumerate() {
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T,
                    SignalKind::Expiry,
                    &format!("m{i}"),
                    name,
                    2
                ),
                BreakerDecision::Pass
            );
        }
    }

    /// Interruptor maestro apagado: cada señal pasa individual, nada se agrupa ni se
    /// suprime (D-72-4).
    #[test]
    fn disabled_breaker_passes_every_signal() {
        let cfg = BreakerConfig {
            enabled: false,
            ..base_cfg()
        };
        let mut breaker = StormBreaker::new(cfg);
        let t0 = Instant::now();

        for (i, name) in ["web", "api", "db", "cache", "worker", "mail"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                breaker.on_signal(
                    t0 + i as u32 * T,
                    SignalKind::Latency,
                    &format!("m{i}"),
                    name,
                    8
                ),
                BreakerDecision::Pass
            );
        }
        assert_eq!(breaker.state(), BreakerState::Closed);
    }

    /// Una señal con una entrada obsoleta detrás de otra refrescada debe podar la
    /// obsoleta: si sobrevive cuenta como monitor y provoca un falso positivo
    /// (misma regresión MAYOR-1, ahora en el episodio de señal).
    #[test]
    fn a_stale_signal_behind_a_refreshed_one_is_pruned() {
        let mut breaker = StormBreaker::new(base_cfg());
        let t0 = Instant::now();

        assert_eq!(
            breaker.on_signal(t0, SignalKind::Expiry, "m1", "tls-a", 8),
            BreakerDecision::Pass
        );
        assert_eq!(
            breaker.on_signal(t0 + T, SignalKind::Expiry, "m2", "tls-b", 8),
            BreakerDecision::Pass
        );
        // m1 (la más antigua) se refresca a los 5 s: m2@1 queda *detrás* de ella.
        assert_eq!(
            breaker.on_signal(t0 + 5_000 * MS, SignalKind::Expiry, "m1", "tls-a", 8),
            BreakerDecision::Pass
        );
        // A los 11.5 s, m2@1 tiene 10.5 s → fuera de la ventana y debe podarse.
        assert_eq!(
            breaker.on_signal(t0 + 11_500 * MS, SignalKind::Expiry, "m3", "tls-c", 8),
            BreakerDecision::Pass
        );
        // Sólo quedan m1 (refrescada a 5 s) y m3: con m4 son tres únicos, por debajo
        // del umbral de cuatro. La obsoleta m2 era el cuarto.
        assert_eq!(
            breaker.on_signal(t0 + 12_000 * MS, SignalKind::Expiry, "m4", "tls-d", 8),
            BreakerDecision::Pass
        );
    }
}
