# PLAN-012 — Canario de red configurable vía settings (#69)

**Issue:** #69 (sub-issue `b` de #62)
**OpenSpec change:** `openspec/changes/canary-interval-setting/`
**Rama:** `feature/canary-interval-setting` (desde `development`)
**Estado:** borrador — pendiente de aprobación de la spec

## Objetivo

Hacer configurable el periodo del canario de red (`netcheck`) vía el setting
`canary_interval_secs` (default 30 s, rango [5, 3600]), aplicable al arrancar y
en caliente, con UI de Ajustes.

## Contexto técnico

- Intervalo fijo hoy: `netcheck::DEFAULT_INTERVAL` (30 s) → `netcheck.rs:22`,
  arranque en `main.rs:118`.
- Settings = tabla clave-valor (`settings(key,value)`, `db.rs:137`); sin
  migración. Persistencia: `get_setting`/`set_setting` (`db.rs:1243,1260`).
- Ruta `POST /api/settings` (`routes/settings.rs:18`) ya recarga el breaker en
  caliente (`scheduler_mgr.reload_breaker()`); patrón a seguir.
- `AppState.netcheck` ya existe (`main.rs:122-131`).
- Frontend `Settings.tsx` usa componentes controlados + `fetchSetting`/`saveSetting`.

## Decisiones de diseño

- **D1 — Recarga en caliente:** sí; el cambio de setting debe aplicar sin
  reinicio (coherente con el breaker). Mecanismo: canal de control en
  `NetworkHealthHandle`; el task del canario adopta el nuevo periodo.
- **D2 — Rango [5, 3600] s:** mínimo 5 s (el `PROBE_TIMEOUT` es 5 s; no tiene
  sentido sondear más rápido que el propio timeout), máximo 3600 s. Default 30.
- **D3 — Sin migración:** el default vive en código (como el breaker), no se
  siembra fila en `settings`.
- **D4 — Sin test de temporización frágil:** la aplicación del intervalo se
  verifica de forma determinista (getter del handle tras `set_interval`), no
  contando ciclos por reloj de pared.

## Tareas

Ver `openspec/changes/canary-interval-setting/tasks.md`.

## Criterios de aceptación

- [ ] Setting persistido con default 30 y validación [5, 3600].
- [ ] Tests: parseo/clamping, round-trip, aplicación del intervalo.
- [ ] `clippy -D warnings` y `fmt` limpios; `tsc --noEmit` si toca frontend.
- [ ] Doc `docs/canary-interval-setting.md`.

## Cierre

- [ ] `openspec archive canary-interval-setting`
- [ ] PR a `development` con `Closes #69`
- [ ] Renombrar a `plans/PLAN-012-ARCHIVED.md`