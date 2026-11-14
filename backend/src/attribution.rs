//! Deterministic attribution of a check failure: target, or our own egress?
//!
//! Phase 3 of PLAN-002. A pure decision table (D5) combines the failure cause
//! (phase 2) with the health of our own connectivity (phase 3) and yields the
//! status that gets persisted and notified. `indeterminate` means "not
//! attributable to the target" and is **not** a failure (D7).
//!
//! Design: `openspec/changes/network-attribution/design.md` (D5, D7, D8).

use crate::checker::{status_for, FailureCause};
use crate::netcheck::NetworkHealth;

/// Status for a failure that cannot be attributed to the target.
pub const INDETERMINATE: &str = "indeterminate";

/// The D5 truth table: `(cause, health) -> status`.
///
/// Pure: no I/O, no clock, no global state, no allocation.
pub fn classify(cause: Option<FailureCause>, health: NetworkHealth) -> &'static str {
    let Some(cause) = cause else {
        // The check succeeded: no failure to attribute.
        return "up";
    };

    // A missing domain is a target configuration problem, whatever our egress
    // looks like: an unreachable network cannot make `NXDOMAIN` disappear.
    if cause == FailureCause::DnsNxdomain {
        return "down";
    }

    match health {
        // No information, or a healthy egress: the target owns the failure.
        NetworkHealth::Unknown | NetworkHealth::Healthy => status_for(Some(cause)),
        // Only the DNS family can be blamed on a degraded DNS path; anything
        // else (a refused connection, a bad status…) reached the target.
        NetworkHealth::DnsDegraded => match cause {
            FailureCause::DnsTimeout | FailureCause::DnsServfail => INDETERMINATE,
            _ => status_for(Some(cause)),
        },
        // Our egress is down: we cannot tell who failed.
        NetworkHealth::Down => INDETERMINATE,
    }
}

/// Apply the truth table to a checker outcome.
///
/// Only a failure can become `indeterminate`: a successful check (`up`, or the
/// TLS checker's `warning`) keeps the status its checker reported. For a failure
/// **with** a cause the verdict is [`classify`]'s — the D5 table is the single
/// source of truth and is not duplicated here.
///
/// Unlike [`classify`], this entry point also attributes failures **without** a
/// cause (M1): a `ping` monitor fails with `cause = None`, yet during an outage
/// of our own egress that failure is just as unattributable as a DNS one.
pub fn attribute(status: &str, cause: Option<FailureCause>, health: NetworkHealth) -> &str {
    // A successful result is never re-attributed: `warning` (expiring TLS
    // certificate) and `up` survive any network health.
    if status == "up" || status == "warning" {
        return status;
    }

    // A missing domain is a target configuration problem whatever our egress
    // looks like: an unreachable network cannot make `NXDOMAIN` disappear.
    if cause == Some(FailureCause::DnsNxdomain) {
        return "down";
    }

    match cause {
        // The table owns the verdict; no second copy of it lives here.
        Some(cause) => classify(Some(cause), health),
        // The table has no row without a cause: only our own egress being down
        // makes such a failure unattributable. A degraded DNS path without a DNS
        // cause still points at the target.
        None => match health {
            NetworkHealth::Down => INDETERMINATE,
            NetworkHealth::Unknown | NetworkHealth::Healthy | NetworkHealth::DnsDegraded => status,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netcheck::NetworkHealth::{DnsDegraded, Down, Healthy, Unknown};

    /// The full D5 truth table, row by row.
    #[test]
    fn classify_should_follow_the_truth_table_for_every_row() {
        let table: [(Option<FailureCause>, NetworkHealth, &str); 24] = [
            // cause `None` (check OK) is `up` for every health.
            (None, Unknown, "up"),
            (None, Healthy, "up"),
            (None, DnsDegraded, "up"),
            (None, Down, "up"),
            // A missing domain is always the target's configuration.
            (Some(FailureCause::DnsNxdomain), Unknown, "down"),
            (Some(FailureCause::DnsNxdomain), Healthy, "down"),
            (Some(FailureCause::DnsNxdomain), DnsDegraded, "down"),
            (Some(FailureCause::DnsNxdomain), Down, "down"),
            // No network information: the base status.
            (Some(FailureCause::ConnectRefused), Unknown, "down"),
            (Some(FailureCause::DnsTimeout), Unknown, "error"),
            // Healthy egress: the base status.
            (Some(FailureCause::ConnectRefused), Healthy, "down"),
            (Some(FailureCause::DnsTimeout), Healthy, "error"),
            (Some(FailureCause::TlsError), Healthy, "down"),
            // Degraded DNS: only DNS failures are unattributable.
            (Some(FailureCause::DnsTimeout), DnsDegraded, INDETERMINATE),
            (Some(FailureCause::DnsServfail), DnsDegraded, INDETERMINATE),
            (Some(FailureCause::ConnectRefused), DnsDegraded, "down"),
            (Some(FailureCause::ConnectTimeout), DnsDegraded, "error"),
            (Some(FailureCause::TlsError), DnsDegraded, "down"),
            (Some(FailureCause::HttpStatus), DnsDegraded, "down"),
            (Some(FailureCause::BodyMismatch), DnsDegraded, "down"),
            (Some(FailureCause::NetworkUnreachable), DnsDegraded, "error"),
            (Some(FailureCause::Other), DnsDegraded, "error"),
            // Egress down: nothing but a missing domain can be attributed.
            (Some(FailureCause::ConnectRefused), Down, INDETERMINATE),
            (Some(FailureCause::DnsTimeout), Down, INDETERMINATE),
        ];

        for (cause, health, expected) in table {
            assert_eq!(
                classify(cause, health),
                expected,
                "cause={cause:?} health={health:?}"
            );
        }
    }

    /// `indeterminate` is a status of its own: it is neither `down` nor `error`.
    #[test]
    fn indeterminate_should_be_neither_down_nor_error() {
        assert_ne!(INDETERMINATE, "down");
        assert_ne!(INDETERMINATE, "error");
        assert_ne!(
            classify(Some(FailureCause::DnsTimeout), DnsDegraded),
            "down"
        );
        assert_ne!(classify(Some(FailureCause::ConnectRefused), Down), "error");
    }

    #[test]
    fn attribute_should_keep_a_successful_status_untouched() {
        assert_eq!(attribute("up", None, Down), "up");
        assert_eq!(attribute("warning", None, DnsDegraded), "warning");
    }

    #[test]
    fn attribute_should_keep_a_target_failure_on_its_base_status() {
        assert_eq!(
            attribute("down", Some(FailureCause::ConnectRefused), Healthy),
            "down"
        );
        assert_eq!(
            attribute("error", Some(FailureCause::DnsTimeout), Healthy),
            "error"
        );
        assert_eq!(
            attribute("down", Some(FailureCause::DnsNxdomain), Down),
            "down"
        );
    }

    #[test]
    fn attribute_should_mark_an_unattributable_failure_as_indeterminate() {
        assert_eq!(
            attribute("error", Some(FailureCause::DnsTimeout), DnsDegraded),
            INDETERMINATE
        );
        assert_eq!(
            attribute("down", Some(FailureCause::ConnectRefused), Down),
            INDETERMINATE
        );
    }

    /// Legacy checkers (and the characterization doubles) report a status
    /// without a cause; the table must not turn those into `up`.
    #[test]
    fn attribute_should_preserve_a_failure_without_cause() {
        assert_eq!(attribute("down", None, Healthy), "down");
        assert_eq!(attribute("error", None, Unknown), "error");
    }

    // ── M1: los fallos sin causa también se atribuyen (cubre `ping`) ──

    /// Scenario: fallo sin causa con la red caída es indeterminado.
    #[test]
    fn attribute_should_mark_a_causeless_failure_as_indeterminate_when_the_network_is_down() {
        assert_eq!(attribute("down", None, Down), INDETERMINATE);
    }

    /// Scenario: fallo sin causa con red sana se atribuye al target.
    #[test]
    fn attribute_should_keep_a_causeless_failure_when_the_network_is_healthy() {
        assert_eq!(attribute("down", None, Healthy), "down");
        assert_eq!(attribute("down", None, Unknown), "down");
    }

    /// A degraded DNS path alone is not enough: without a DNS cause the failure
    /// still belongs to the target.
    #[test]
    fn attribute_should_keep_a_causeless_failure_when_only_dns_is_degraded() {
        assert_eq!(attribute("down", None, DnsDegraded), "down");
    }

    /// Scenario: éxito sin causa no se altera.
    #[test]
    fn attribute_should_never_reattribute_a_successful_status() {
        assert_eq!(attribute("up", None, Down), "up");
        assert_eq!(attribute("warning", None, Down), "warning");
    }

    // ── N2: `attribute` no duplica la tabla D5, delega en `classify` ──

    /// Every cause of the taxonomy, so the table-driven test below cannot silently
    /// skip a variant.
    const ALL_CAUSES: [FailureCause; 10] = [
        FailureCause::DnsNxdomain,
        FailureCause::DnsTimeout,
        FailureCause::DnsServfail,
        FailureCause::ConnectRefused,
        FailureCause::ConnectTimeout,
        FailureCause::NetworkUnreachable,
        FailureCause::TlsError,
        FailureCause::HttpStatus,
        FailureCause::BodyMismatch,
        FailureCause::Other,
    ];

    /// For `Some(cause)` the verdict is the D5 table's, for every combination of
    /// cause and health. `attribute` must not keep a second copy of the table: the
    /// equivalence with `classify` is checked even against a `status` that drifts
    /// from the canonical `status_for(cause)`, which is what makes the drift
    /// observable instead of silent.
    #[test]
    fn attribute_should_delegate_to_classify_for_every_cause_and_health() {
        let healths = [Unknown, Healthy, DnsDegraded, Down];

        for cause in ALL_CAUSES {
            let canonical = status_for(Some(cause));
            for health in healths {
                let expected = classify(Some(cause), health);
                assert_eq!(
                    attribute(canonical, Some(cause), health),
                    expected,
                    "canonical status drifted: cause={cause:?} health={health:?}"
                );
                assert_eq!(
                    attribute("bogus", Some(cause), health),
                    expected,
                    "the table must be the single source of truth: \
                     cause={cause:?} health={health:?}"
                );
            }
        }
    }

    /// The `cause = None` rows (legacy checkers, `ping`): only a down egress makes
    /// the failure unattributable, and a success is never re-attributed.
    #[test]
    fn attribute_should_handle_every_health_without_a_cause() {
        for health in [Unknown, Healthy, DnsDegraded] {
            assert_eq!(
                attribute("down", None, health),
                "down",
                "without a cause only a down egress is unattributable: {health:?}"
            );
            assert_eq!(attribute("error", None, health), "error", "{health:?}");
        }
        assert_eq!(attribute("down", None, Down), INDETERMINATE);

        for health in [Unknown, Healthy, DnsDegraded, Down] {
            assert_eq!(attribute("up", None, health), "up", "{health:?}");
            assert_eq!(attribute("warning", None, health), "warning", "{health:?}");
        }
    }
}
