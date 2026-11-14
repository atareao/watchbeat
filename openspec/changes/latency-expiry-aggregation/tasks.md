# Tasks

## 1. RED — tests que fallan

- [x] 1.1 Test unitario en `breaker.rs`: ráfaga de `latency` de N monitores distintos dentro de la ventana → `OpenAggregate` con la lista de monitores.
- [x] 1.2 Test unitario: señal de `latency` tras el rearme (ventana sin nuevas señales) vuelve a agrupar/no agrupa según umbral.
- [x] 1.3 Test unitario: `Suppress` para señales de `latency` dentro del mismo episodio.
- [x] 1.4 Test unitario: independencia por tipo — una tormenta de `latency` no suprime `expiry` ni `down`.
- [x] 1.5 Test unitario: `min_monitors` / un solo monitor que oscila nunca declara tormenta de señal.
- [x] 1.6 Test unitario: `enabled=false` → todas las señales `Pass`.
- [x] 1.7 Test de scheduler: enrutado de `latency`/`expiry` por `on_signal` (Pass individual / Suppress / OpenAggregate agregado).
- [x] 1.8 `cargo test` en RED (fallan los nuevos, verdes los existentes).

## 2. GREEN — implementación mínima

- [x] 2.1 `SignalKind { Latency, Expiry }` y almacenamiento de episodios por tipo en `StormBreaker`.
- [x] 2.2 `on_signal(now, kind, id, name, enabled_monitors) -> BreakerDecision` con umbral/ventana compartidos y rearme perezoso por tipo.
- [x] 2.3 `record_signal` con refresco in-place por monitor (reutiliza la lógica MAYOR-1) y `prune` por tipo.
- [x] 2.4 `template.rs`: `defaults::STORM_LATENCY` y `defaults::STORM_EXPIRY`.
- [x] 2.5 `scheduler.rs`: `latency`/`expiry` pasan por `on_signal`; render del agregado a la unión de notificadores.
- [x] 2.6 `cargo test` y `cargo check` verdes.

## 3. REFACTOR

- [x] 3.1 Extraer la lógica común de episodios (down vs señal) sin duplicar.
- [x] 3.2 `cargo fmt --check` y `cargo clippy --all-targets --all-features -- -D warnings` limpios.
- [x] 3.3 Re-ejecutar `cargo test`.

## 4. Cierre

- [ ] 4.1 `docs/latency-expiry-aggregation.md`.
- [ ] 4.2 `@rust-reviewer` sobre el diff.
- [ ] 4.3 PR a `development` con `Closes #72`.
- [ ] 4.4 Archivar change en `openspec/changes/archive/` y marcar `plans/PLAN-015.md` → `-ARCHIVED.md`.
