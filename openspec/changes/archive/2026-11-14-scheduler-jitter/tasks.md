To check an item, replace `[ ]` with `[x]`.

## Preparación
- [x] Issue #38 creado
- [x] Change proposal OpenSpec creado
- [x] Rama `feature/scheduler-jitter`

## RED — Tests
- [x] Desplazamiento inicial en `[0, intervalo)` (RNG inyectado)
- [x] Jitter por ciclo en `[0.9·I, 1.1·I]` (RNG inyectado)
- [x] Cota inferior de espera (≥ 1 s) para intervalos pequeños
- [x] Reproducibilidad con RNG determinista
- [x] El `interval_seconds` configurado no cambia
- [x] Monitores con el mismo intervalo quedan desfasados (no coinciden)
- [x] Un check lento no genera ráfaga

## GREEN — Implementación
- [x] Helper de jitter con RNG inyectable (nuevo módulo o en `scheduler.rs`)
- [x] Añadir dependencia `rand` a `backend/Cargo.toml`
- [x] Sustituir `tokio::time::interval` por el bucle secuencial con jitter en `monitor_task_inner`
- [x] Aplicar el desplazamiento inicial antes del primer check
- [x] Aplicar el jitter por ciclo a la espera entre checks

## REFACTOR
- [x] `cargo clippy --all-targets --all-features -- -D warnings` limpio
- [x] `cargo fmt --check` limpio

## Documentación
- [x] Crear `docs/scheduler-jitter.md`

## Cierre
- [x] `cargo test` verde
- [x] PR a `development` (`Closes #38`)
- [x] `openspec archive scheduler-jitter`
- [x] Actualizar `plans/PLAN-002.md`
