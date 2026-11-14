//! Tick jitter for the per-monitor scheduler loop (Fase 4 of PLAN-002, D1–D4).
//!
//! With `tokio::time::interval` every monitor fires its first tick immediately, so
//! N monitors started together run their first check in the same second
//! (*thundering herd*): simultaneous connections, SQLite writes and network
//! bursts — and, on a brief DNS/network hiccup, simultaneous failures. Two
//! components break that up:
//!
//! - an **initial offset**, uniform in `[0, interval)`, which de-synchronises
//!   monitors sharing an interval and keeps them apart for good (D1);
//! - a **per-cycle jitter** of ±10 %, which stops them from converging again
//!   after a restart or an `Update` (D2).
//!
//! The randomness is injectable (D4), and so is the wait itself (D3), so both the
//! computation and the loop's ordering can be tested deterministically.
//!
//! Design: `openspec/changes/scheduler-jitter/design.md`.

use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;
use rand::{Rng, RngExt};

/// Hard floor for the wait between two checks (D2): whatever the configured
/// interval says, a target is never hammered faster than this.
pub const MIN_DELAY: Duration = Duration::from_secs(1);

/// Per-cycle jitter, as a percentage of the interval (D2: ±10 %).
const CYCLE_JITTER_PERCENT: u64 = 10;

/// Milliseconds in `duration`, saturating instead of wrapping.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Closed millisecond window of the per-cycle wait, `[0.9·I, 1.1·I]` (D2).
fn cycle_window_ms(interval: Duration) -> (u64, u64) {
    let base = millis(interval);
    let spread = base / CYCLE_JITTER_PERCENT;
    (base - spread, base + spread)
}

/// Uniform initial offset in `[0, interval)`, applied before the first check (D1).
///
/// Monitors configured with the same interval draw different offsets, which is
/// what keeps them de-synchronised. A zero interval has an empty range, so the
/// offset is zero: there is nothing to spread over.
pub fn initial_offset<R: Rng + ?Sized>(interval: Duration, rng: &mut R) -> Duration {
    let span_ms = millis(interval);
    if span_ms == 0 {
        return Duration::ZERO;
    }

    Duration::from_millis(rng.random_range(0..span_ms))
}

/// Uniform wait in `[0.9·I, 1.1·I]`, never below [`MIN_DELAY`] (D2).
///
/// The result depends only on the interval and the RNG — never on how long the
/// previous check took, which is what keeps a slow check from producing a burst.
pub fn next_delay<R: Rng + ?Sized>(interval: Duration, rng: &mut R) -> Duration {
    let (low_ms, high_ms) = cycle_window_ms(interval);
    let drawn_ms = if low_ms == high_ms {
        // A degenerate window holds a single value: never sample an empty range.
        low_ms
    } else {
        rng.random_range(low_ms..=high_ms)
    };

    Duration::from_millis(drawn_ms).max(MIN_DELAY)
}

/// One iteration of a monitor's cycle (D3).
///
/// The production implementation runs the real check and keeps its bookkeeping;
/// tests implement it with a fake, which is how the loop's ordering is asserted
/// without waiting for real time.
#[async_trait]
pub(crate) trait Cycle {
    /// Run one check, updating whatever bookkeeping the implementation owns.
    async fn check(&mut self);
}

/// Drive `cycle` forever, with jitter (D1–D3).
///
/// The wait is counted **after** each check completes, so a check that outlives
/// its interval delays the next one instead of queueing catch-up ticks. That is
/// what makes the old `MissedTickBehavior::Skip` unnecessary (D3).
///
/// `sleep` is injected for the same reason as the RNG: it lets the tests drive
/// the loop on a fake clock.
pub(crate) async fn jittered_cycle<C, S, SF, R>(
    interval: Duration,
    rng: &mut R,
    cycle: &mut C,
    mut sleep: S,
) where
    C: Cycle + ?Sized,
    S: FnMut(Duration) -> SF,
    SF: Future<Output = ()>,
    R: Rng + ?Sized,
{
    sleep(initial_offset(interval, rng)).await;
    loop {
        cycle.check().await;
        // Counted after the check, so an overrunning check delays the next one
        // instead of queueing catch-up ticks (D3).
        sleep(next_delay(interval, rng)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::sync::{Arc, Mutex};

    const MINUTE: Duration = Duration::from_secs(60);

    /// Seeded draws: every assertion below is deterministic (D4).
    fn rng(seed: u64) -> StdRng {
        StdRng::seed_from_u64(seed)
    }

    // ── Scenario: El desplazamiento inicial está dentro del intervalo ──

    #[test]
    fn initial_offset_should_stay_within_the_interval() {
        let mut rng = rng(11);

        assert_eq!(
            initial_offset(Duration::ZERO, &mut rng),
            Duration::ZERO,
            "a zero interval has no room to spread over"
        );

        for interval in [
            Duration::from_millis(1),
            Duration::from_secs(1),
            MINUTE,
            Duration::from_secs(3600),
        ] {
            for _ in 0..1000 {
                let offset = initial_offset(interval, &mut rng);
                assert!(
                    offset < interval,
                    "{offset:?} must be inside [0, {interval:?})"
                );
            }
        }
    }

    // ── Scenario: El jitter por ciclo está en ±10 % ──

    #[test]
    fn the_cycle_window_should_be_plus_minus_ten_percent() {
        assert_eq!(cycle_window_ms(MINUTE), (54_000, 66_000));
        assert_eq!(cycle_window_ms(Duration::from_secs(1)), (900, 1_100));
        assert_eq!(
            cycle_window_ms(Duration::from_secs(3600)),
            (3_240_000, 3_960_000)
        );
    }

    #[test]
    fn next_delay_should_stay_within_the_cycle_window() {
        let mut rng = rng(12);

        for interval in [Duration::from_secs(2), MINUTE, Duration::from_secs(3600)] {
            let (low, high) = cycle_window_ms(interval);
            let (mut min, mut max) = (u64::MAX, 0u64);

            for _ in 0..2000 {
                let delay = millis(next_delay(interval, &mut rng));
                assert!(
                    delay >= low && delay <= high,
                    "{delay} ms is outside [{low}, {high}] for interval {interval:?}"
                );
                min = min.min(delay);
                max = max.max(delay);
            }

            assert!(
                max - min > (high - low) / 2,
                "the draws must spread over the window, got [{min}, {max}] of [{low}, {high}]"
            );
        }
    }

    // ── Scenario: Cota inferior de la espera ──

    #[test]
    fn next_delay_should_never_go_below_the_floor() {
        let mut rng = rng(13);

        for interval in [
            Duration::ZERO,
            Duration::from_millis(1),
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(2),
        ] {
            for _ in 0..500 {
                let delay = next_delay(interval, &mut rng);
                assert!(
                    delay >= MIN_DELAY,
                    "{delay:?} is below the floor for interval {interval:?}"
                );
            }
        }

        assert_eq!(next_delay(Duration::ZERO, &mut rng), MIN_DELAY);
    }

    // ── Scenario: Monitores con el mismo intervalo no arrancan a la vez ──

    #[test]
    fn monitors_sharing_an_interval_should_not_start_together() {
        let mut rng = rng(14);
        let interval = MINUTE;

        let offsets: Vec<Duration> = (0..8).map(|_| initial_offset(interval, &mut rng)).collect();

        assert!(
            offsets.iter().all(|offset| *offset < interval),
            "every offset must be inside the interval: {offsets:?}"
        );

        let mut seconds: Vec<u64> = offsets.iter().map(|offset| offset.as_secs()).collect();
        seconds.sort_unstable();
        seconds.dedup();
        assert!(
            seconds.len() > 1,
            "8 monitors must not cron in the same second: {offsets:?}"
        );

        let min = offsets.iter().min().copied().unwrap_or_default();
        let max = offsets.iter().max().copied().unwrap_or_default();
        assert!(
            max - min > interval / 4,
            "the offsets must be spread over the interval: {offsets:?}"
        );
    }

    // ── Scenario: Cálculo reproducible con aleatoriedad inyectada ──

    #[test]
    fn the_jitter_should_be_reproducible_with_a_seeded_rng() {
        let interval = MINUTE;
        let draw = |seed: u64| {
            let mut rng = rng(seed);
            let mut values = vec![millis(initial_offset(interval, &mut rng))];
            for _ in 0..5 {
                values.push(millis(next_delay(interval, &mut rng)));
            }
            values
        };

        let first = draw(15);
        assert_eq!(
            first,
            draw(15),
            "the same seed must reproduce the same jitter"
        );
        assert_ne!(
            first,
            draw(16),
            "a different seed must produce a different sequence"
        );

        assert!(
            first[0] < millis(interval),
            "the offset is inside the interval"
        );
        assert!(
            first[1..]
                .iter()
                .all(|value| (54_000..=66_000).contains(value)),
            "every wait is inside ±10 %: {first:?}"
        );
    }

    // ── Scenario: Un check lento no genera ráfaga ──

    /// What the fake cycle did, on the fake clock (D3).
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Step {
        /// A wait of `wait` started when the clock read `at`.
        Wait { wait: Duration, at: Duration },
        /// A check finished when the clock read `at`.
        Check { at: Duration },
    }

    struct FakeClock {
        now: Duration,
    }

    /// A cycle whose every check overruns the interval.
    struct SlowCycle {
        clock: Arc<Mutex<FakeClock>>,
        log: Arc<Mutex<Vec<Step>>>,
        duration: Duration,
        runs: usize,
        max_runs: usize,
    }

    #[async_trait]
    impl Cycle for SlowCycle {
        async fn check(&mut self) {
            self.runs += 1;
            if self.runs > self.max_runs {
                // The loop only ends when aborted: the test wraps it in a timeout.
                std::future::pending::<()>().await;
            }

            let mut clock = self.clock.lock().unwrap();
            clock.now += self.duration;
            self.log.lock().unwrap().push(Step::Check { at: clock.now });
        }
    }

    #[tokio::test]
    async fn a_slow_check_should_not_generate_a_burst() {
        let interval = Duration::from_secs(10);
        let clock = Arc::new(Mutex::new(FakeClock {
            now: Duration::ZERO,
        }));
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut rng = rng(17);

        let mut cycle = SlowCycle {
            clock: Arc::clone(&clock),
            log: Arc::clone(&log),
            // Five times the interval: every check overruns it by far.
            duration: interval * 5,
            runs: 0,
            max_runs: 3,
        };

        let sleep_clock = Arc::clone(&clock);
        let sleep_log = Arc::clone(&log);
        let sleep = move |wait: Duration| {
            let at = {
                let mut clock = sleep_clock.lock().unwrap();
                let at = clock.now;
                clock.now += wait;
                at
            };
            sleep_log.lock().unwrap().push(Step::Wait { wait, at });
            std::future::ready(())
        };

        let result = tokio::time::timeout(
            Duration::from_millis(50),
            jittered_cycle(interval, &mut rng, &mut cycle, sleep),
        )
        .await;
        assert!(
            result.is_err(),
            "the fake cycle hangs after its last run, so the timeout must fire"
        );

        let steps = log.lock().unwrap().clone();
        assert_eq!(
            steps.len(),
            7,
            "the initial offset plus three (check, wait) pairs: {steps:?}"
        );

        let (mut checks, mut waits) = (0, 0);
        for (i, step) in steps.iter().enumerate() {
            match step {
                Step::Wait { wait, at } => {
                    waits += 1;
                    if i == 0 {
                        assert!(
                            *wait < interval,
                            "the initial offset is inside the interval"
                        );
                        assert_eq!(*at, Duration::ZERO, "the offset is the very first wait");
                    } else {
                        assert!(
                            *wait >= interval * 9 / 10 && *wait <= interval * 11 / 10,
                            "every cycle wait is the full jittered delay, got {wait:?}"
                        );
                        if i >= 2 {
                            assert_eq!(
                                steps[i - 1],
                                Step::Check { at: *at },
                                "the wait starts when the check finishes, never before"
                            );
                        }
                    }
                }
                Step::Check { .. } => {
                    checks += 1;
                    assert!(i > 0, "the initial offset precedes the first check");
                    assert!(
                        matches!(steps[i - 1], Step::Wait { .. }),
                        "exactly one check per wait, no catch-up: {steps:?}"
                    );
                }
            }
        }

        assert_eq!(
            checks, 3,
            "a check that overruns its interval must not queue extra checks: {steps:?}"
        );
        assert_eq!(
            waits, 4,
            "the initial offset plus one wait per check: {steps:?}"
        );
    }
}
