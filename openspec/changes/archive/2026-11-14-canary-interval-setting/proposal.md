# Proposal

**Issue:** [#69](https://github.com/atareao/watchbeat/issues/69)
**Plan:** `plans/PLAN-012.md`

## Why

El canario de red (`backend/src/netcheck.rs`) sondea nuestro propio egress cada
`DEFAULT_INTERVAL` = 30 s **fijo en código** (`netcheck.rs:22`), aplicado en
`main.rs:118` al arrancar (`netcheck::spawn(netcheck::DEFAULT_INTERVAL)`). No hay
forma de ajustar la cadencia sin recompilar y reiniciar: en redes con egress
lento un canario demasiado agresivo añade ruido, y en despliegues críticos 30 s
puede ser demasiado lento para atribuir un fallo. Los ajustes del storm breaker
ya son configurables y recargables en caliente (`routes/settings.rs` →
`scheduler_mgr.reload_breaker()`); el canario es el único componente de robustez
que aún no lo es. Es el bloque `b` de #62.

## What Changes

- Nuevo setting persistido `canary_interval_secs` (entero en texto), **default 30**, con validación de rango **[5, 3600]** segundos (clamping).
- `netcheck` expone una función pura `interval_from_setting(Option<&str>) -> Duration`
  que parsea, aplica default ante ausencia/valor inválido y clampa al rango.
- El intervalo se resuelve desde `settings` al arrancar (`main.rs`).
- **Recarga en caliente**: `POST /api/settings` con `canary_interval_secs` aplica el
  nuevo intervalo al canario en ejecución, sin reinicio (vía `AppState.netcheck`).
- UI de Settings: nuevo control (pestaña "Red") para ver/guardar el intervalo.
- Tests unitarios de parseo/clamping, de round-trip settings, y de aplicación del
  intervalo al handle.

## Capabilities

### Added Capabilities

- `netcheck`: el intervalo del canario SHALL ser configurable vía settings, con
  default 30 s y validación de rango, aplicable en caliente.
- `settings`: nuevo setting `canary_interval_secs`.

## Impact

- `backend/src/netcheck.rs` (consts + `interval_from_setting` + `NetworkHealthHandle`
  con control de intervalo + tests).
- `backend/src/main.rs` (resolver el intervalo desde la BD al arrancar).
- `backend/src/routes/settings.rs` (recargar el canario al cambiar el setting).
- `frontend/src/pages/Settings.tsx` (control del intervalo).
- Sin migración de esquema: `settings` es clave-valor (`CREATE TABLE ... settings`).
- Sin cambios de contrato de API existente (se añade una clave nueva).