# Perf: queries lentas en checks DB

**Issue:** [#33](https://github.com/atareao/watchbeat/issues/33)

## Problema

La DB SQLite de checks ha acumulado 300k registros con 7 monitores.
Tres queries en `backend/src/db.rs` son lentas y, al concurrir con los
checks del scheduler, provocan bloqueos sqlite y errores falsos en la UI.

| Query | Tiempo actual | Causa |
|---|---|---|
| `get_checks_filtered` — WHERE status IN ORDER BY DESC LIMIT OFFSET | 4.9s | Índice ASC no cubre DESC |
| `get_timeline_buckets` — baja todas las filas a Rust | 3.5s | Agregación en Rust, no en SQL |
| `calculate_uptime` — COUNT/SUM sobre 21k filas | 1.1s | Mismo índice ASC |

## Solución

1. **Índice DESC:** `CREATE INDEX idx_checks_timeline ON checks(monitor_id, checked_at DESC, status)`.
2. **Bucketing SQL:** Reemplazar `get_timeline_buckets` por query con GROUP BY.
3. **Migración:** Añadir el índice en `init_db` tras los índices existentes.

## Scope

- **Solo `backend/src/db.rs`** — `init_db` y `get_timeline_buckets`.
- Sin cambios en llamadores, frontend, scheduler, ni rutas.
- Sin nueva dependencia.
