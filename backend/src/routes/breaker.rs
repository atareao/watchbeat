//! `GET /api/breaker` — estado y configuración vigentes del storm breaker.
//!
//! El breaker vive dentro del scheduler (una única instancia global compartida
//! por todos los monitores); este módulo solo lo **observa**, leyendo el handle
//! que expone [`crate::scheduler::SchedulerManager::breaker`]. No lo muta ni lo
//! persiste: el estado arranca siempre `closed` y su ciclo de vida lo gobierna el
//! scheduler.
//!
//! El estado se reporta **tal cual lo tiene el breaker**: el rearme es perezoso
//! (ocurre en la siguiente transición `on_down`/`on_up`), así que un `open` sin
//! transiciones posteriores puede quedar vigente más allá de la ventana. Es el
//! estado real sobre el que decide el scheduler, no una proyección calculada.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::auth::AppState;

/// Snapshot del breaker expuesto por la API: estado + configuración vigente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BreakerStatus {
    /// `"closed"` (todo pasa) o `"open"` (tormenta: se suprimen las individuales).
    pub state: String,
    /// Interruptor maestro del breaker.
    pub enabled: bool,
    /// Umbral `X`, en porcentaje de los monitores habilitados.
    pub threshold_percent: u8,
    /// Ventana de detección `Y`, en segundos.
    pub window_seconds: u64,
    /// Suelo absoluto del umbral: por debajo de esto nunca hay tormenta.
    pub min_monitors: usize,
}

/// `GET /api/breaker` (protegido) → `{"status": BreakerStatus}`.
pub async fn get_breaker(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, String> {
    let (breaker_state, cfg) = {
        let breaker = state.scheduler_mgr.breaker();
        let guard = breaker.lock().await;
        (guard.state(), guard.config())
    };

    let status = BreakerStatus {
        state: breaker_state.as_str().to_string(),
        enabled: cfg.enabled,
        threshold_percent: cfg.threshold_percent,
        window_seconds: cfg.window_seconds,
        min_monitors: cfg.min_monitors,
    };

    Ok(Json(serde_json::json!({ "status": status })))
}
