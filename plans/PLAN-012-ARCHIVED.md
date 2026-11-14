# PLAN-012 — Canario de red configurable vía settings (#69)

**Issue:** #69 (sub-issue `b` de #62)
**PR:** #74
**OpenSpec change:** `canary-interval-setting` (archivado en `openspec/changes/archive/2026-11-14-canary-interval-setting/`)
**Rama:** `feature/canary-interval-setting`
**Estado:** COMPLETADO

## Objetivo

Hacer configurable el periodo del canario de red (`netcheck`) vía el setting
`canary_interval_secs` (default 30 s, rango [5, 3600]), aplicable al arrancar y
en caliente, con UI de Ajustes.

## Contexto técnico

- Intervalo fijo antes: `netcheck::DEFAULT_INTERVAL` (30 s) → `netcheck.rs`, arranque en `main.rs`.
- Settings = tabla clave-valor (`settings(key,value)`); sin migración.
- Ruta `POST /api/settings` ya recargaba el breaker en caliente (`reload_breaker()`); patrón seguido.
- `AppState.netcheck` ya existía.

## Decisiones de diseño

- **D1 — Recarga en caliente:** sí (coherente con el breaker); `watch::Sender` + `select!`.
- **D2 — Rango [5, 3600] s**, default 30.
- **D3 — Sin migración:** default en código, no se siembra fila en `settings`.
- **D4 — Test determinista** de adopcación con tokio `start_paused` (sin reloj de pared).

## Tareas

Ver `openspec/changes/archive/2026-11-14-canary-interval-setting/tasks.md` (todas completadas).

## Criterios de aceptación

- [x] Setting persistido con default 30 y validación [5, 3600].
- [x] Tests: parseo/clamping, round-trip, aplicación del intervalo.
- [x] `clippy -D warnings` y `fmt` limpios; `tsc --noEmit` limpio.
- [x] Doc `docs/canary-interval-setting.md`.

## Cierre

- [x] `openspec archive canary-interval-setting`
- [x] PR #74 a `development` con `Closes #69`
- [x] Renombrado a `plans/PLAN-012-ARCHIVED.md`