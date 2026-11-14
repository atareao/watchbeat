# Proposal

**Issue:** [#71](https://github.com/atareao/watchbeat/issues/71)
**Plan:** `plans/PLAN-014.md`

## Why

El storm breaker (`backend/src/breaker.rs`) decide si una tormenta de caídas debe agregarse en una única alerta y si se suprimen las notificaciones individuales, pero su estado (`closed`/`open`) no es observable desde fuera: se crea dentro de `manager_loop` (`scheduler.rs:153`) como `Arc<Mutex<StormBreaker>>` y no se expone en `AppState` ni en ninguna API. Operadores y usuarios no pueden saber si el sistema está en medio de una tormenta, ni si las notificaciones individuales están siendo suprimidas. `BreakerState::as_str()` ya se escribió pensando en «future API». Es el bloque `d` de #62.

## What Changes

- El breaker pasa a ser **compartido** con `AppState`: se construye en `SchedulerManager` y se expone con un getter; `manager_loop` usa ese mismo handle.
- Nuevo endpoint protegido `GET /api/breaker` que devuelve `{"status":{"state":"closed|open","enabled":..,"threshold_percent":..,"window_seconds":..,"min_monitors":..}}`, leyendo el estado real del breaker.
- Frontend: el dashboard muestra un badge con el estado del breaker y, mientras está abierto, las tarjetas de monitor indican que las notificaciones individuales están suprimidas por tormenta. Degradación no bloqueante si el fetch falla.

**Alcance:** el breaker es único y global (no hay uno por monitor); se expone el estado global y se refleja en las vistas de monitor.

## Capabilities

### Added Capabilities

- `breaker`: el estado del breaker SHALL exponerse en un endpoint protegido (`GET /api/breaker`) con la config vigente, y el dashboard SHALL reflejarlo.

## Impact

- `backend/src/breaker.rs` (posible getter/estado ya existente) y `backend/src/scheduler.rs` (crear el breaker en `SchedulerManager` y compartirlo con `manager_loop`).
- `backend/src/routes/` (`breaker.rs` o `status.rs`) + registro en el router `protected`; `BreakerStatus` en `models.rs`.
- `frontend/src/api/http.ts`, `frontend/src/components/BreakerBadge.tsx` (nuevo), `frontend/src/pages/Dashboard.tsx`, `frontend/src/components/MonitorCard.tsx`.
- `docs/breaker-state.md`.
- Sin migración de esquema (el estado no se persiste; arranca `Closed`).
