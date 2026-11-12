# Checks Performance — Optimización de queries

## Problema original

Tras 30 días de operación con 7 monitores, la DB SQLite acumuló **300k checks** (134 MB). Tres queries en `db.rs` se volvieron lentas (1-5s) bloqueando el scheduler:

| Query | Tiempo | Causa |
|---|---|---|
| `get_checks_filtered` — WHERE status IN ORDER BY DESC | 4.9s | Índice ASC, sort en Rust |
| `get_timeline_buckets` — 21k filas a Rust | 3.5s | Agregación en Rust |
| `calculate_uptime` — COUNT/SUM sobre 21k filas | 1.1s | Volumen |

## Solución aplicada

### 1. Bucketing en SQL (en vez de en Rust)

`get_timeline_buckets` ahora ejecuta una query SQL con `GROUP BY bucket_start` que devuelve ~60 filas (los buckets) en lugar de bajar las 21k filas con `get_timeline()` y agruparlas en Rust.

**Query:**
```sql
SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 
       CAST((CAST(strftime('%s', checked_at) / ? AS INTEGER) * ?) AS TEXT), 
       'unixepoch') AS bucket_start,
       100.0 * CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS FLOAT) / COUNT(*),
       AVG(CAST(response_time_ms AS FLOAT)),
       COUNT(*)
FROM checks WHERE monitor_id=? AND checked_at>=?
GROUP BY bucket_start ORDER BY bucket_start
```

### 2. Sin índice redundante

Tras analizar con `EXPLAIN QUERY PLAN`, se determinó que el índice existente `idx_checks_uptime(monitor_id, checked_at, status)` cubre adecuadamente las queries. No se añadió índice DESC — SQLite escanea índices ASC en orden inverso sin penalización.

### 3. Retención reducida (en producción)

En `co1` se redujo `retention_days` de 30 a 14 días (vía settings DB). DB pasó de 134 MB a 56 MB.

## Archivos afectados

- `backend/src/db.rs`: `get_timeline_buckets` reescrita (31 líneas nuevas, 50 eliminadas)

## No se tocó

- `get_checks_filtered`, `get_timeline`, `calculate_uptime`, `get_latest_check` — mejoran automáticamente al reducir volumen de datos
- Frontend, scheduler, rutas, llamadores — firma de `get_timeline_buckets` idéntica

## Lecciones aprendidas

1. **Siempre verificar queries SQL con sqlite3 real** antes de darlas por buenas — errores como `'utc'` vs `'unixepoch'` no los detecta el compilador.
2. **Contar `?` vs `.bind()`**: sqlx-sqlite no da error si sobran binds (trata los no bindeados como NULL).
3. **`AVG` ignora NULLs en SQLite**, igual que el filter_map original en Rust — semánticamente equivalente.

## Referencias

- Issue [#33](https://github.com/atareao/watchbeat/issues/33)
- PLAN-001 (archivado)
- Change proposal `checks-perf`
