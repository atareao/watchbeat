To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: test de plan de ejecución para `get_checks_filtered` (usa `idx_checks_timeline`, sin sort)
- [x] RED: test de plan de ejecución para `get_timeline_buckets` (covering index)
- [x] RED: test de plan de ejecución para `calculate_uptime` (covering index)
- [x] RED: test de índices existentes/eliminados en `sqlite_master`
- [x] RED: test de `ANALYZE` (`sqlite_stat1` poblada)
- [x] RED: smoke de rendimiento con 20k filas (timeline < 100 ms)
- [x] GREEN: crear `idx_checks_timeline` y eliminar índices redundantes en `init_db`
- [x] GREEN: ejecutar `ANALYZE` tras las migraciones
- [x] REFACTOR: clippy/fmt limpios, tests verdes

## Cierre

- [x] `docs/checks-query-perf.md`
- [x] `openspec archive checks-query-perf`
- [x] PR a `development` con `Closes #33`
