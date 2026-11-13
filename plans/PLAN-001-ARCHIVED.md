# PLAN-001: Optimización de queries lentas en checks DB

**Issue:** [#33](https://github.com/atareao/watchbeat/issues/33)
**Estado:** Pendiente

## Objetivo
Eliminar las queries lentas (>1s) en `backend/src/db.rs` que causan errores falsos en los monitores cuando el scheduler y el frontend compiten por sqlite.

## Cambios

### 1. Índice DESC compuesto
Añadir migración en `init_db` para crear:
```sql
CREATE INDEX IF NOT EXISTS idx_checks_timeline ON checks(monitor_id, checked_at DESC, status)
```
Cubre `get_checks_filtered`, `get_timeline`, `get_latest_check`, `calculate_uptime` con index-only scan sin sort.

### 2. Bucketing en SQL
Reemplazar `get_timeline_buckets` — que hoy baja todas las filas con `get_timeline()` (21k) y las agrupa en Rust — por una query SQL directa con GROUP BY:
```sql
SELECT CAST(STRFTIME('%s', checked_at) / ? AS INTEGER) * ? * 1000 AS ts,
       COUNT(*) AS count,
       CAST(SUM(CASE WHEN status='up' THEN 1 ELSE 0 END) AS INTEGER) AS up_count
FROM checks WHERE monitor_id=? AND checked_at>=?
GROUP BY ts ORDER BY ts
```
Esto devuelve 60 filas en vez de 21k.

### 3. Migración de schema
En `init_db`, dentro del bloque de creación de tablas, añadir el CREATE INDEX después de los índices existentes.

## Archivos afectados
- `backend/src/db.rs`: `init_db` (+1 línea), `get_timeline_buckets` (~30 líneas de cambio)

## No se toca
- `get_checks_filtered`, `get_timeline`, `calculate_uptime`: mejoran automáticamente con el nuevo índice
- `get_timeline_buckets` es private/hacia dentro; ningún llamador cambia

## Verificación
1. `cargo test` — todos verdes
2. `just check` — clippy + fmt limpio
3. Los índices existentes no se tocan (solo se añade uno nuevo)

## Tareas

- [ ] Issue #33 creado
- [ ] Change proposal OpenSpec creado
- [ ] PLAN-001 escrito
- [ ] RED: tests para el nuevo índice y bucketing
- [ ] GREEN: migración índice + bucketing SQL
- [ ] REFACTOR: clippy + fmt
- [ ] docs/checks-perf.md escrito
- [ ] Merge a development
- [ ] PLAN-001 → PLAN-001-ARCHIVED