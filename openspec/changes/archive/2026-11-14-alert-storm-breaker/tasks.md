To check an item, replace `[ ]` with `[x]`.

## Preparación
- [x] Issue #39 (ya creado)
- [x] Change proposal OpenSpec creado
- [x] Rama `feature/alert-storm-breaker`

## RED — Tests
- [x] Umbral: N monitores caen en la ventana → `OpenAggregate` (una alerta)
- [x] Pocas caídas aisladas → `Pass` (individual)
- [x] Umbral absoluto mínimo (`min_monitors`) con pocos monitores → nunca tormenta
- [x] Dedupe: durante `Open` no se repite la agregada
- [x] Rearme tras la ventana sin caídas → nuevo episodio vuelve a alertar
- [x] Supresión de `up` individual durante `Open`
- [x] `storm_breaker_enabled = false` → siempre `Pass`
- [x] Umbral 100 % exige el 100 % de los monitores
- [x] Determinismo con reloj inyectado (sin sleeps)

## GREEN — Implementación
- [x] `backend/src/breaker.rs` (config, estados, decisión, alerta agregada, máquina de estados)
- [x] `backend/src/lib.rs`: `pub mod breaker`
- [x] `backend/src/template.rs`: `defaults::STORM` + contexto de tormenta
- [x] `backend/src/scheduler.rs`: `Arc<Mutex<StormBreaker>>` compartido; punto de decisión en el despacho
- [x] `backend/src/scheduler.rs`: `SchedulerCommand::ReloadBreaker` + `SchedulerManager::reload_breaker`
- [x] `backend/src/routes/settings.rs`: emitir `ReloadBreaker` al guardar claves del breaker
- [x] `BreakerConfig::load(&db)` desde settings con defaults
- [x] Frontend: pestaña "Tormenta" en `Settings.tsx` (enabled, X %, Y s, min, plantilla)

## REFACTOR
- [x] `cargo clippy --all-targets --all-features -- -D warnings` limpio
- [x] `cargo fmt --check` limpio
- [x] `npx tsc --noEmit` limpio

## Documentación
- [x] Crear `docs/alert-storm-breaker.md`

## Cierre
- [x] `cargo test` verde
- [x] PR a `development` (`Closes #39`)
- [x] `openspec archive alert-storm-breaker`
- [x] Actualizar `plans/PLAN-002.md`
