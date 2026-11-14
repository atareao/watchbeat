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
| 5 · Breaker / anti-tormenta | #39 | ⏳ pendiente |
| 6 · Documentación final + razones | #40 | ⏳ pendiente |

## Última fase cerrada — Fase 4 (jitter)

- `backend/src/jitter.rs` (nuevo): desplazamiento inicial uniforme en `[0, intervalo)`
  + jitter por ciclo ±10 % (cota ≥ 1 s); RNG y reloj inyectables.
- `scheduler.rs`: `monitor_task_inner` usa `jittered_cycle`; bucle secuencial con
  `sleep` sustituye a `tokio::time::interval` (anti-ráfagas).
- Dep `rand = "0.10"`. Spec `scheduler` actualizada. Doc `docs/scheduler-jitter.md`.
- Tests: lib 212 + api_keys 9 + db_integration 45 + run_check_route 3. Clippy/fmt limpios.
- Auditoría `@rust-reviewer`: APROBADO.

## Próximo paso

Arrancar **Fase 5 · Breaker / anti-tormenta (#39)**: un change proposal OpenSpec
nuevo, TDD RED-GREEN-REFACTOR, PR a `development` con `Closes #39`.

## Contexto técnico clave

- Estados de `checks.status`: `up`, `warning`, `down`, `error`, `indeterminate`.
- `indeterminate` = observabilidad, no incidente (no notifica, no toca `was_up`).
- Rechazados por diseño: reintentos/`confirmations_required` como atribución.
- Git Flow: features desde `development`, PR con `Closes #NN`, `--merge`.
