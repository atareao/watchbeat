To check an item, replace `[ ]` with `[x]`.

## Tareas

### Preparación
- [x] Issue #33 creado
- [x] Change proposal OpenSpec creado
- [x] PLAN-001 escrito

### RED — Tests
- [x] Test unitario: `init_db` crea el índice `idx_checks_timeline`
- [x] Test integración: `get_timeline_buckets` devuelve ~60 buckets con datos reales
- [x] Test integración: `get_timeline_buckets` con monitor inexistente → vacío
- [x] Test integración: `get_timeline_buckets` con rango vacío → vacío

### GREEN — Implementación
- [x] Añadir CREATE INDEX en `init_db` tras índices existentes
- [x] Reemplazar `get_timeline_buckets` con query GROUP BY SQL

### REFACTOR
- [x] `cargo clippy -- -D warnings` limpio
- [x] `cargo fmt --check` limpio

### Documentación
- [x] Crear `docs/checks-perf.md`

### Cierre
- [x] Merge feature branch a development
- [x] PLAN-001 → PLAN-001-ARCHIVED
