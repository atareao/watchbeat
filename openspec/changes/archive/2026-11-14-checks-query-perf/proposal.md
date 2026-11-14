# Proposal

**Issue:** [#33](https://github.com/atareao/watchbeat/issues/33)
**Plan:** `plans/PLAN-003.md`

## Why

La tabla `checks` acumula ~300k filas. La spec `db` ya exige un índice
`idx_checks_timeline(monitor_id, checked_at DESC, status)` que **nunca se creó**:
el código solo tiene `idx_checks_monitor(monitor_id, checked_at DESC)` y
`idx_checks_uptime(monitor_id, checked_at, status)` (ASC, sin `response_time_ms`).
Como consecuencia, `get_timeline_buckets` hace *table lookups* por fila para leer
`response_time_ms` y tarda ~477 ms en 300k filas (medido), y el planner no tiene
estadísticas (`ANALYZE`) para elegir bien entre índices.

## What Changes

- **Índice cubriente** `idx_checks_timeline(monitor_id, checked_at DESC, status, response_time_ms)`.
  Sirve a `get_checks`, `get_checks_filtered`, `get_latest_check`, `get_timeline`,
  `get_timeline_buckets` y `calculate_uptime` sin *table lookups*.
- **Eliminar índices redundantes** `idx_checks_monitor` e `idx_checks_uptime`
  (el nuevo índice los subsume como prefijo), reduciendo el coste de escritura.
- **`ANALYZE`** tras las migraciones para refrescar las estadísticas del planner.
- **Tests** de plan de ejecución (`EXPLAIN QUERY PLAN`) y smoke de rendimiento.

## Capabilities

### New Capabilities
_(ninguna)_

### Modified Capabilities

- `db`: el requisito del índice de timeline se amplía con `response_time_ms` y se
  añade un requisito de estadísticas del planner.

## Impact

- Solo `backend/src/db.rs` (migración de índices) y tests.
- Sin cambios de API ni de esquema de tablas.
- Las DB existentes migran al arrancar (`CREATE INDEX IF NOT EXISTS` + `DROP INDEX IF EXISTS`).
