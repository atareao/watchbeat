# HANDOFF — Diagnóstico y optimización de queries lentas

## Estado actual (13 Nov 2026)

### Problema original
WatchBeat en Co1 mostraba errores esporádicos en todos los monitores. Causa raíz: la DB SQLite acumuló **300k checks (134 MB)** y 3 queries en `backend/src/db.rs` tardaban 1-5s. Al concurrir con los checks del scheduler, sqlite bloqueaba → timeouts → falsos positivos.

### Medidas aplicadas

| Medida | Dónde | Detalle |
|---|---|---|
| Retención 14d (urgente) | Co1 — settings DB | Manual via sqlite3 en Co1 el 12 Nov |
| Bucketing SQL | `backend/src/db.rs` — `get_timeline_buckets` | GROUP BY SQL en vez de bajar 21k filas a Rust (3.5s → ~60ms) |
| Opciones retención | `frontend/src/pages/Settings.tsx` | Select: 7 y 14 días (en vez de 7, 15, 30) |
| CI simplificado | `.github/workflows/docker-multiarch.yml` | Solo amd64, sin QEMU/arm64 |
| Docs | `docs/checks-perf.md` | Detalle técnico del cambio |

### Releases
- `v0.14.1` — bucketing SQL + CI amd64 (desplegado en Co1)
- `v0.14.2` — opciones retención 7/14 días (desplegado en Co1)

### Para revisar más adelante

1. **¿Siguen apareciendo errores falsos?** Si el problema persiste, hay que investigar si el scheduler sigue teniendo timeouts por concurrencia sqlite (buscar en logs de Co1 `slow statement` o `error` en los checks).
2. **Retención automática:** Verificar que el cleanup `cleanup_old_checks` se ejecuta cada 24h (no hay log específico de cleanup — si se necesita, añadir un `tracing::info!`).
3. **Migración del compose en Co1:** El compose de producción está en `/home/lorenzo/docker/dockge/stacks/watchbeat/`, gestionado por Dockge. La raíz del repo tiene `compose.yml` de ejemplo.
4. **Próximo paso opcional:** Keyset pagination en `get_checks` y `get_checks_filtered` para eliminar `OFFSET` en tablas grandes (issue [#33](https://github.com/atareao/watchbeat/issues/33) abierto).

### Archivos relevantes
- `backend/src/db.rs` (get_timeline_buckets reescrita)
- `docs/checks-perf.md` (documentación técnica)
- `openspec/specs/db/spec.md` (spec consolidada)
- `openspec/specs/settings/spec.md` (spec retención)
