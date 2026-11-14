# PLAN-015 — Agrupar notificaciones latency/expiry en el breaker

**Issue:** [#72](https://github.com/atareao/watchbeat/issues/72) (sub-issue `e` de #62)
**Change OpenSpec:** `openspec/changes/latency-expiry-aggregation/`
**Estado:** EN CURSO — pendiente de aprobación de la spec (Fase 1 SDD)

## Objetivo

Que las notificaciones `latency` y `expiry` pasen por la lógica de agrupación del storm
breaker, de modo que una ráfaga de avisos (degradación de red/proveedor) se coaleszca en una
única alerta agregada y se supriman las individuales hasta el rearme, sin cambios de esquema.

## Diseño

- `SignalKind { Latency, Expiry }` y `StormBreaker::on_signal(now, kind, id, name, enabled_monitors) -> BreakerDecision`.
- Umbral/ventana compartidos con la tormenta de caídas; contabiliza **monitores distintos**.
- Episodios independientes por tipo con rearme perezoso (ventana `Y`).
- `Pass` → individual · `Suppress` → descartada · `OpenAggregate` → una agregada por episodio.
- Plantillas `defaults::STORM_LATENCY` / `defaults::STORM_EXPIRY`; agregado a la unión de notificadores.
- `enabled=false` → comportamiento clásico.

## Tareas

Ver checklist TDD en `openspec/changes/latency-expiry-aggregation/tasks.md`.

## Verificación

- `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`.
- `openspec validate latency-expiry-aggregation --strict`.
- PR a `development` con `Closes #72`.

## Cierre

- `docs/latency-expiry-aggregation.md`.
- Archivar change y renombrar este plan a `PLAN-015-ARCHIVED.md`.
