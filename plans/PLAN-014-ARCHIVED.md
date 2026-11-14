# PLAN-014 — Estado del breaker (closed/open) en API y dashboard (#71)

- **Issue:** [#71](https://github.com/atareao/watchbeat/issues/71) (sub-issue bloque **d** de #62)
- **Change OpenSpec:** `breaker-state`
- **Rama:** `feature/breaker-state`
- **Estado:** 🟡 Esperando aprobación de la spec (SDD)

## Objetivo

Exponer el estado real del storm breaker (`closed`/`open`) y su configuración vigente en un endpoint protegido, y reflejarlo en el dashboard (badge global + indicador en cada `MonitorCard` mientras está abierto).

**Alcance decidido por el usuario:** el breaker es único y global (no hay un breaker por monitor), así que se expone **el estado global**; «por monitor» se interpreta como «visible en las vistas de monitor».

## Contexto / hallazgos (auditados)

- `backend/src/breaker.rs`: enum `BreakerState { Closed, Open }` con `as_str() -> "closed"|"open"` (el comentario ya dice «future API»); `BreakerConfig { enabled: bool, threshold_percent: u8, window_seconds: u64, min_monitors: usize }`; `StormBreaker { cfg, state, events }`; métodos públicos `state()`, `config()`, `configure(cfg)`.
- **El breaker NO está en `AppState`**: se crea dentro de `manager_loop` (`scheduler.rs:153`) como `Arc<Mutex<StormBreaker>>` y se clona a cada `monitor_task`. `SchedulerManager` no expone getter.
- **Patrón a imitar:** `NetworkHealthHandle` (en `netcheck.rs`) vive en `AppState` (`auth.rs:184`) con lectura no bloqueante.
- Settings del breaker: `storm_breaker_enabled`, `storm_threshold_percent`, `storm_window_seconds`, `storm_min_monitors`; recarga en caliente vía `SchedulerCommand::ReloadBreaker` (`routes/settings.rs`, `routes/export_import.rs`).
- Endpoints hoy: `GET /health` (público), `GET /api/status` (protegido → `"{\"status\": DashboardStatus}"`), `GET /api/monitors` (protegido → `UnifiedDashboardResponse`). Ninguno incluye breaker ni red.
- Registro de rutas: `public` vs `protected` en `routes/mod.rs`; middleware `require_auth` con lista blanca (`/auth/*`, `/health`, `/`, `/api/heartbeat/*`, `/api/events`); el resto de `/api/*` exige Bearer.
- Frontend: NO hay indicador de breaker ni de red. `Dashboard.tsx` hace stat cards, SSE (`useSse`) + fallback poll 60 s (`setInterval(refresh, 60_000)`). `api/http.ts` tiene un `fetcher<T>` genérico. Componentes existentes: `AppLayout.tsx`, `MonitorCard.tsx`. No hay tests de frontend (verificación: `tsc --noEmit` + build).
- OpenSpec: el estado del breaker solo se describe en `openspec/specs/breaker/spec.md`; NO existe spec de dashboard/frontend.

## Contrato de API (nuevo)

`GET /api/breaker` (protegido, JWT) → `200`:

```json
{\"status\":{\"state\":\"closed\",\"enabled\":true,\"threshold_percent\":40,\"window_seconds\":300,\"min_monitors\":3}}
```

- `state`: `"closed" | "open"` (de `BreakerState::as_str()`), leído del breaker **real y compartido** con el scheduler.
- El resto refleja la `BreakerConfig` vigente.
- Sin token válido → `401`.

## Diseño

- **Compartir el breaker:** crear el `Arc<Mutex<StormBreaker>>` en `SchedulerManager` (al construirlo, resolviendo `BreakerConfig::load(&db)`), exponerlo con `pub fn breaker(&self) -> Arc<...>`, y que `manager_loop` use el clone compartido en lugar de crear el suyo. `AppState` ya contiene `scheduler_mgr`, así que basta con eso.
- **Endpoint:** nuevo módulo `routes/breaker.rs` (o extender `routes/status.rs`) con `GET /api/breaker`; registrar en el router `protected`; modelo `BreakerStatus { state, enabled, threshold_percent, window_seconds, min_monitors }` (Serialize).
- **Frontend:** tipo `BreakerStatus` + `fetchBreaker()` en `api/http.ts`; nuevo componente `components/BreakerBadge.tsx`; `Dashboard.tsx` lo consume en `load()`/`refresh()` y renderiza el badge; `MonitorCard.tsx` recibe `breakerOpen` y muestra un indicador sutil (con tooltip) cuando está abierto. Degradación no bloqueante si el fetch falla.

## Fases y tareas (TDD)

1. **RED:** test del endpoint — `GET /api/breaker` devuelve `200` con `state:"closed"` y la config; y tras forzar `Open` (vía el handle) devuelve "open"; y `401` sin token.
2. **GREEN:** compartir el breaker en `SchedulerManager`; añadir `BreakerStatus`, la ruta `GET /api/breaker` y su registro en `protected`.
3. **REFACTOR:** `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, suite completa verde.
4. **Frontend:** `fetchBreaker()` + tipo; `BreakerBadge`; integración en `Dashboard`; indicador en `MonitorCard`; `npx tsc --noEmit` + `npm run build`.
5. **Doc y cierre:** `docs/breaker-state.md`; `openspec archive breaker-state`; PR a `development` con `Closes #71`.

## Verificación
- Backend: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`.
- Frontend: `npx tsc --noEmit`, `npm run build`.
- SDD: `openspec validate breaker-state --strict`.
- Auditoría: `@rust-reviewer` + `@react-reviewer`.

## Riesgos / notas
- El breaker debe seguir creándose/leyéndose igual para el scheduler (no romper el orden con `BreakerConfig::load` async).
- NO exponer el breaker en endpoints públicos; `/api/breaker` debe quedar en `protected`.
- La UI no debe romperse ni parpadear si el fetch del breaker falla → degradar a «desconocido».
- Issue: [#71](https://github.com/atareao/watchbeat/issues/71).
- **Rearme perezoso:** `state:"open"` puede quedar obsoleto si cesan las transiciones (el breaker sólo reevalúa el cierre en `on_down`/`on_up`). Documentado en `docs/breaker-state.md`; alternativa futura: `snapshot(now)` que rearme al leer.