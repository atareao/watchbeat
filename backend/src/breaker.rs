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
    /// Number of **unique** monitors that went down inside the window.
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

/// A down transition observed by the breaker: when it happened and to whom.
type DownEvent = (Instant, String, String);

/// Pure, clock-injected state machine detecting alert storms.
pub struct StormBreaker {
    cfg: BreakerConfig,
    state: BreakerState,
    /// Down transitions still inside the window, oldest first.
    events: VecDeque<DownEvent>,
}

impl StormBreaker {
    /// Build a breaker in the `Closed` state with the given configuration.
    pub fn new(cfg: BreakerConfig) -> Self {
        Self {
            cfg,
            state: BreakerState::Closed,
            events: VecDeque::new(),
        }
    }

    /// Apply a new configuration in place (hot reload). Disabling the breaker
    /// closes it and forgets the current episode.
    pub fn configure(&mut self, cfg: BreakerConfig) {
        self.cfg = cfg;
        if !cfg.enabled {
            self.state = BreakerState::Closed;
            self.events.clear();
        }
    }

    /// The configuration currently in use.
    pub fn config(&self) -> BreakerConfig {
        self.cfg
    }

    /// The current state.
    pub fn state(&self) -> BreakerState {
        self.state
    }

    /// Report a monitor going down and get the decision for this transition.
    pub fn on_down(
        &mut self,
        now: Instant,
        monitor_id: &str,
        monitor_name: &str,
        enabled_monitors: usize,
    ) -> BreakerDecision {
        // Disabled: classic behaviour, nothing is aggregated nor suppressed.
        if !self.cfg.enabled {
            return BreakerDecision::Pass;
        }

        // Lazy rearm (D2): a storm that saw no new down for a whole window is over
        // by the time of this call.
        self.maybe_close(now);

        self.record_down(now, monitor_id, monitor_name);

        // Storm in progress: the aggregated alert is already out (D4).
        if self.state == BreakerState::Open {
            return BreakerDecision::Suppress;
        }

        // Too few enabled monitors to ever declare a storm (D3).
        if enabled_monitors < self.cfg.min_monitors {
            return BreakerDecision::Pass;
        }

        // The threshold counts monitors, not transitions (MAYOR-1): `events` holds
        // one entry per unique monitor inside the window.
        let threshold = self.threshold(enabled_monitors);
        if self.events.len() >= threshold {
            self.state = BreakerState::Open;
            BreakerDecision::OpenAggregate(self.aggregate(threshold))
        } else {
            BreakerDecision::Pass
        }
    }

    /// Report a monitor recovering and get the decision for this transition.
    pub fn on_up(&mut self, now: Instant) -> BreakerDecision {
        if !self.cfg.enabled {
            return BreakerDecision::Pass;
        }

        self.maybe_close(now);
        if self.state == BreakerState::Open {
            BreakerDecision::Suppress
        } else {
            BreakerDecision::Pass
        }
    }

    /// Absolute threshold for the given number of enabled monitors:
    /// `max(min_monitors, ceil(threshold_percent · enabled / 100))`.
    fn threshold(&self, enabled_monitors: usize) -> usize {
        let by_percent = (enabled_monitors * self.cfg.threshold_percent as usize).div_ceil(100);
        by_percent.max(self.cfg.min_monitors)
    }

    /// The detection window as a duration.
    fn window(&self) -> Duration {
        Duration::from_secs(self.cfg.window_seconds)
    }

    /// Record a down transition, keyed by monitor.
    ///
    /// The threshold counts **monitors**, not transitions (MAYOR-1): when the same
    /// monitor is already inside the window its event is refreshed in place (new
    /// instant and name) instead of being pushed again, so one flapping monitor can
    /// never declare a storm on its own and silence the rest of the fleet.
    fn record_down(&mut self, now: Instant, monitor_id: &str, monitor_name: &str) {
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

    /// Drop the events that fell out of the window.
    ///
    /// An event exactly `window_seconds` old is already *out*: the boundary is
    /// closed, which is the same criterion [`Self::maybe_close`] uses to rearm
    /// (NIT-2).
    ///
    /// `retain` instead of popping from the front: [`Self::record_down`] refreshes
    /// events **in place**, so the deque is no longer sorted by instant and a stale
    /// entry can sit behind a fresher one (REGRESIÓN MAYOR-1).
    fn prune(&mut self, now: Instant) {
        let window = self.window();
        self.events
            .retain(|(at, _, _)| now.saturating_duration_since(*at) < window);
    }

    /// Lazily rearm the breaker once the window elapsed without new downs.
    fn maybe_close(&mut self, now: Instant) {
        self.prune(now);
        if self.state != BreakerState::Open {
            return;
        }

        let window = self.window();
        // The latest down is the maximum instant, not `back()`: in-place refreshes
        // mean the deque is not sorted (REGRESIÓN MAYOR-1).
        let rearmed = match self.events.iter().map(|(at, _, _)| *at).max() {
            Some(last) => now.saturating_duration_since(last) >= window,
            // No event left inside the window: the episode is over.
            None => true,
        };

        if rearmed {
            self.state = BreakerState::Closed;
            self.events.clear();
        }
    }

    /// Snapshot of the current episode as an aggregated alert.
    ///
    /// `events` already holds one entry per unique monitor, so both the count and
    /// the lists describe *distinct* monitors (MAYOR-1).
    fn aggregate(&self, threshold: usize) -> AggregateAlert {
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
            window_seconds: self.cfg.window_seconds,
            threshold,
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
}
