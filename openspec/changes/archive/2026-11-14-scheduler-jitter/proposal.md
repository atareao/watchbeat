# Proposal

**Issue:** [#38](https://github.com/atareao/watchbeat/issues/38)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 4)

## Why

Todos los monitores se despachan con `tokio::time::interval` arrancando su primer
tick de inmediato, así que **cronan en el mismo segundo**. Con 8 monitores (y en
general N), eso produce un *thundering herd*: picos simultáneos de conexiones,
escrituras SQLite y tráfico de red, incluso cuando cada monitor va bien. Además,
cuando el DNS/red del host sufre un microcorte, ese alineamiento hace que todos
fallen a la vez.

## What Changes

- El ciclo de cada monitor incorpora **jitter**:
  - un **desplazamiento inicial** uniforme en `[0, intervalo)` que desincroniza
    los monitores con el mismo intervalo;
  - un **jitter por ciclo** de ±10 % del intervalo que evita que vuelvan a
    converger.
- El **RNG es inyectable** para tests deterministas.
- El ciclo pasa a ser un bucle secuencial basado en `sleep`, preservando la
  propiedad anti-ráfagas (un check lento retrasa el siguiente; equivale al
  `MissedTickBehavior::Skip` actual).
- **No cambia** el `interval_seconds` configurado/persistido, ni la API, ni el
  resto del comportamiento de los checks.

## Capabilities

### New Capabilities
_(ninguna)_

### Modified Capabilities
- `scheduler`: el ciclo de cada monitor pasa a estar desincronizado por jitter.

## Impact

- **Código**: `backend/src/scheduler.rs` (ciclo de `monitor_task_inner`), nuevo
  módulo/helper de jitter con RNG inyectable, `backend/Cargo.toml` (dependencia
  `rand`).
- **Sin** migración, **sin** cambios de API/frontend, **sin** nuevas variables
  obligatorias.
