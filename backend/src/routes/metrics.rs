use axum::body::Body;
use axum::response::Response;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

use std::sync::{Mutex, OnceLock};

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry::default()))
}

fn checks_total() -> &'static Counter {
    static M: OnceLock<Counter> = OnceLock::new();
    M.get_or_init(|| {
        let c = Counter::default();
        registry()
            .lock()
            .unwrap()
            .register("watchbeat_checks", "Total checks", c.clone());
        c
    })
}

fn monitors_up() -> &'static Gauge {
    static M: OnceLock<Gauge> = OnceLock::new();
    M.get_or_init(|| {
        let g = Gauge::default();
        registry()
            .lock()
            .unwrap()
            .register("watchbeat_monitors_up", "Monitors up", g.clone());
        g
    })
}

fn monitors_down() -> &'static Gauge {
    static M: OnceLock<Gauge> = OnceLock::new();
    M.get_or_init(|| {
        let g = Gauge::default();
        registry()
            .lock()
            .unwrap()
            .register("watchbeat_monitors_down", "Monitors down", g.clone());
        g
    })
}

pub fn inc_checks() {
    checks_total().inc();
}

pub fn set_monitor_counts(up: u64, down: u64) {
    monitors_up().set(up as i64);
    monitors_down().set(down as i64);
}

pub async fn metrics_handler() -> Response {
    let mut buf = String::new();
    encode(&mut buf, &registry().lock().unwrap()).unwrap();
    Response::builder()
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(buf))
        .unwrap()
}