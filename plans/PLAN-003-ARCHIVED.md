# PLAN-003 · Perf de queries de checks (#33)

- **Issue**: [#33](https://github.com/atareao/watchbeat/issues/33)
- **Rama**: `feature/checks-query-perf`
- **Change OpenSpec**: `openspec/changes/checks-query-perf/`

## Contexto

La tabla `checks` acumula ~300k filas. Tres queries eran lentas (1–5 s) y, al
concurrir con los checks del scheduler, provocaban timeouts y falsos errores.

Estado real medido (DB sintética, 300k filas, un monitor):

| Query | Antes (issue) | Hoy (código actual) | Objetivo |
|---|---|---|---|
| `get_checks_filtered` (status IN + DESC) | 4.9 s | ~3 ms | < 10 ms |
| `get_timeline_buckets` (30 d) | 3.5 s | ~477 ms | < 100 ms |
| `calculate_uptime` (30 d) | 1.1 s | ~17 ms | < 50 ms |

El bucketing en SQL ya se implementó (commit `52eca412`). Lo que falta es el
**índice que la spec `db` ya exige** (`idx_checks_timeline`) y que nunca se creó,
más estadísticas del planner.

## Objetivos

1. Crear `idx_checks_timeline(monitor_id, checked_at DESC, status, response_time_ms)`
   — cubriente para timeline (status + response_time_ms) y para checks filtrados.
2. Eliminar los índices redundantes `idx_checks_monitor` e `idx_checks_uptime`
   (el nuevo índice los subsume como prefijo).
3. Ejecutar `ANALYZE` tras las migraciones para que el planner tenga estadísticas.
4. Tests que verifiquen el **plan de ejecución** (deterministas) y un smoke de
   rendimiento con umbrales holgados.

## No objetivos

- Reintroducir tablas de rollup (`consolidated_metrics` se eliminó a propósito).
- Cambiar el esquema de `checks` (no hay columna epoch nueva).
- Keyset pagination: el `OFFSET` profundo mide ~1 ms; no es el cuello de botella.

## Tareas (TDD)

- [x] RED: tests de plan de ejecución + smoke de perf (deben fallar).
- [x] GREEN: índices + `ANALYZE` en `init_db`.
- [x] REFACTOR: limpiar índices redundantes, clippy/fmt.
- [x] Docs: `docs/checks-query-perf.md`.
- [x] PR a `development` con `Closes #33`.
