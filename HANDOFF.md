# HANDOFF — Robust Check Engine (PLAN-002)

## Estado actual (14 Nov 2026)

Objetivo: blindar WatchBeat para que las **caídas de infraestructura/DNS del host**
nunca se confundan con caídas reales de los servicios.

- Issue paraguas: [#34](https://github.com/atareao/watchbeat/issues/34)
- Plan: `plans/PLAN-002.md` (6 fases, una a una)
- Bug colateral: [#41](https://github.com/atareao/watchbeat/issues/41) (rustls CryptoProvider)

## Fases

| Fase | Issue | Estado |
|---|---|---|
| 1 · Resolver DNS propio | #35 | ✅ mergeada (PR #42) |
| 2 · Taxonomía `FailureCause` + `error_kind` | #36 | ✅ mergeada (PR #43) |
| 3 · Canario `NetworkHealth` + atribución `indeterminate` | #37 | ✅ mergeada (PR #44) |
| 4 · Jitter de ticks | #38 | ✅ mergeada (PR #45) |
| 5 · Breaker / anti-tormenta | #39 | ✅ mergeada (PR #46) |
| 6 · Documentación final + razones | #40 | ⏳ pendiente |

## Última fase cerrada — Fase 5 (breaker / anti-tormenta)

- `backend/src/breaker.rs` (nuevo): `StormBreaker` (máquina de estados pura, reloj
  inyectado), `BreakerConfig` (settings KV), dedupe por monitor único y por episodio.
- `scheduler.rs`: `Arc<Mutex<StormBreaker>>` compartido, decisión en el despacho,
  `ReloadBreaker`, unión de notificadores de los afectados.
- `template.rs`: `defaults::STORM` + `TemplateContext::for_storm`.
- `routes/settings.rs` + `routes/export_import.rs`: recarga en caliente.
- `frontend/src/pages/Settings.tsx`: pestaña **Tormenta**.
- Spec `breaker` (nueva). Doc `docs/alert-storm-breaker.md`.
- Tests: lib 238 + api_keys 9 + db_integration 45 + run_check_route 3. Clippy/fmt/tsc/build limpios.
- Auditoría `@rust-reviewer` (2 rondas) + `@react-reviewer`: APROBADO.

## Próximo paso

Arrancar **Fase 6 · Documentación integral y cierre (#40)**: `docs/robust-check-engine.md`
(visiones + razones), ADR de decisiones clave, actualizar README/CHANGELOG/env example,
informe final, cerrar #35-#40 y #34, renombrar `plans/PLAN-002.md` → `-ARCHIVED`.

## Contexto técnico clave

- Estados de `checks.status`: `up`, `warning`, `down`, `error`, `indeterminate`.
- `indeterminate` = observabilidad, no incidente (no notifica, no toca `was_up`).
- Rechazados por diseño: reintentos/`confirmations_required` como atribución.
- Git Flow: features desde `development`, PR con `Closes #NN`, `--merge`.
