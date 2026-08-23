use axum::response::Response;
use axum::body::Body;
use prometheus::{Encoder, TextEncoder, Counter, Gauge};

use std::sync::OnceLock;

fn registry() -> &'static prometheus::Registry {
    static REG: OnceLock<prometheus::Registry> = OnceLock::new();
    REG.get_or_init(prometheus::Registry::new)
}

fn checks_total() -> &'static Counter {
    static M: OnceLock<Counter> = OnceLock::new();
    M.get_or_init(|| {
        let c = Counter::new("vigilatrs_checks_total", "Total checks").unwrap();
        registry().register(Box::new(c.clone())).unwrap();
        c
    })
}

fn monitors_up() -> &'static Gauge {
    static M: OnceLock<Gauge> = OnceLock::new();
    M.get_or_init(|| {
        let g = Gauge::new("vigilatrs_monitors_up", "Monitors up").unwrap();
        registry().register(Box::new(g.clone())).unwrap();
        g
    })
}

fn monitors_down() -> &'static Gauge {
    static M: OnceLock<Gauge> = OnceLock::new();
    M.get_or_init(|| {
        let g = Gauge::new("vigilatrs_monitors_down", "Monitors down").unwrap();
        registry().register(Box::new(g.clone())).unwrap();
        g
    })
}

/// Increment the checks counter. Called from scheduler after each check.
pub fn inc_checks() { checks_total().inc(); }

/// Set monitor counts. Called from scheduler loop.
pub fn set_monitor_counts(up: u64, down: u64) {
    monitors_up().set(up as f64);
    monitors_down().set(down as f64);
}

pub async fn metrics_handler() -> Response {
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    let families = registry().gather();
    encoder.encode(&families, &mut buffer).unwrap();
    Response::builder()
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(buffer))
        .unwrap()
}