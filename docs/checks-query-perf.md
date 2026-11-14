# Perf de queries de checks (#33)

- **Issue**: [#33](https://github.com/atareao/watchbeat/issues/33)
- **Plan**: `plans/PLAN-003.md`
- **Change OpenSpec**: `openspec/changes/archive/…-checks-query-perf/`

## Qué es

Optimización de las queries que leen la tabla `checks` (hasta ~300k filas):
historial filtrado por estado, timeline por buckets y cálculo de uptime.

## Por qué

Tres queries tardaban entre 1 y 5 s y, al concurrir con los checks del scheduler,
provocaban timeouts y falsos errores en todos los monitores. El bucketing en SQL
ya se había implementado (commit `52eca412`), pero la spec `db` exigía un índice
`idx_checks_timeline` que **nunca se creó**. El código solo tenía:

- `idx_checks_monitor(monitor_id, checked_at DESC)` — sin `status` ni `response_time_ms`.
- `idx_checks_uptime(monitor_id, checked_at, status)` — ASC y sin `response_time_ms`.

El timeline lee `status` y `response_time_ms`, así que cada fila requería un
*table lookup*: 477 ms en 300k filas (medido).

## Qué se hizo

### Índice cubriente único

```sql
CREATE INDEX IF NOT EXISTS idx_checks_timeline
    ON checks(monitor_id, checked_at DESC, status, response_time_ms);
```

Sirve a **todas** las queries por monitor:

| Query | Uso del índice |
|---|---|
| `get_checks` / `get_latest_check` | prefijo `(monitor_id, checked_at DESC)` |
| `get_checks_filtered` | filtro `status` + orden DESC sin sort |
| `get_timeline` | rango `checked_at` |
| `get_timeline_buckets` | **covering** (`status` + `response_time_ms`) |
| `calculate_uptime` | **covering** (`status`) |

### Índices redundantes eliminados

`idx_checks_monitor` e `idx_checks_uptime` quedan subsumidos por el prefijo del
nuevo índice y se eliminan (`DROP INDEX IF EXISTS`) para no pagar escrituras
duplicadas en cada `INSERT` de check.

### Estadísticas del planner

`init_db` ejecuta `ANALYZE` tras las migraciones. Sin estadísticas, el planner
puede elegir un índice peor; con ellas elige `idx_checks_timeline` de forma
consistente.

## Resultados medidos

DB sintética, un monitor, `cargo test` (debug; SQLite bundled compilado sin
optimizar, ~3× más lento que release):

| Query | Antes | Después |
|---|---|---|
| `get_checks_filtered` (status IN + DESC) | ~3 ms | ~3 ms (sin sort) |
| `get_timeline_buckets` (21k filas, 14 d) | ~33 ms | **< 100 ms** (test) |
| `get_timeline_buckets` (300k filas, 30 d) | 477 ms | **201 ms** |
| `calculate_uptime` (30 d) | ~17 ms | ~17 ms (covering) |

> El objetivo del issue (< 100 ms) se cumple para el escenario real de 21k filas
> por monitor. El caso extremo de 300k filas en un solo monitor baja de 477 a
> 201 ms; el resto del coste es el `GROUP BY` sobre una expresión calculada.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/db.rs` | Índice nuevo, `DROP INDEX` de los redundantes, `ANALYZE` |
| `backend/tests/query_perf.rs` | Tests de plan de ejecución + smoke de rendimiento |
| `backend/Cargo.toml` | `sqlx` como dev-dependency (tests abren una conexión auxiliar) |

## Cómo verificar

```bash
cd backend && cargo test --test query_perf
```

Los tests comprueban, de forma determinista:

1. `idx_checks_timeline` existe y los redundantes no.
2. `get_checks_filtered` usa el índice y **no** ordena con temp b-tree.
3. Timeline y uptime usan el índice como **COVERING INDEX**.
4. `ANALYZE` puebla `sqlite_stat1`.
5. Timeline sobre 21k filas < 100 ms.

## Decisiones

- **Un índice en vez de tres**: el prefijo `(monitor_id, checked_at DESC)` cubre
  las queries existentes; añadir `status` y `response_time_ms` lo hace cubriente
  para timeline/uptime. Menos índices = menos coste de escritura.
- **`ANALYZE` en el arranque**: coste despreciable (~decenas de ms) frente al
  riesgo de que el planner elija mal.
- **Sin columna epoch**: agrupar por `strftime('%s', checked_at)` cuesta CPU, pero
  añadir una columna + backfill + cambio del write path no compensa para el
  escenario real (21k filas). Queda como follow-up si el volumen crece.

## Follow-ups

- Si un monitor supera ~100k filas, evaluar una columna `checked_at_epoch`
  (medido: 66 ms vs 201 ms en 300k filas) o una tabla de rollup.
- `PRAGMA optimize` periódico en lugar de `ANALYZE` en cada arranque.
