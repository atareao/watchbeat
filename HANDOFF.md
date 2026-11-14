# Handoff — watchbeat — 2026-11-14 (sesión #72)

## Objetivo
Cerrar los follow-ups del Robust Check Engine (#62) uno por sesión, con SDD+TDD, dejando cada feature en un PR a `development`.

## Estado
- Hecho:
  - **#69** canario de red configurable — PR #74 mergeado.
  - **#70** `error_kind` en el evento SSE — PR #75 mergeado.
  - **#71** estado del storm breaker en API y dashboard — PR #76 mergeado.
  - **#72** agrupación de notificaciones latency/expiry en el breaker — PR #77 abierto a `development` (commit `9e015bdf`).
  - Suite: 379 tests backend verdes; `cargo fmt --check` y `cargo clippy -- -D warnings` limpios; `openspec validate latency-expiry-aggregation --strict` OK.
- Rama actual: `feature/latency-expiry-aggregation` (base `development`). PRs abiertos: #77.

## Pendiente
1. Mergear PR #77 a `development`.
2. Tras el merge: `openspec archive latency-expiry-aggregation` y renombrar `plans/PLAN-015.md` → `plans/PLAN-015-ARCHIVED.md`.
3. Después: **#63** (♿ a11y del timeline + test SNI real).

## Verificación
```bash
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
openspec validate latency-expiry-aggregation --strict
```

## Metodología / reglas
- SDD+TDD: change OpenSpec aprobado por el usuario ANTES de escribir código; luego RED→GREEN→REFACTOR verificado por CLI.
- Gitflow: `feature/*` desde `development`; **PR a `development`**; nunca commits directos a `main`/`development`.
- Commits con gitmoji + conventional commits.
- Planes en `plans/PLAN-<NNN>.md` (siguiente libre: PLAN-016); al cerrar → renombrar a `-ARCHIVED.md`.
- Docs por feature en `docs/<feature>.md`. El **issue es el punto de entrada** (crear/asignar a `atareao`).
- Header del delta OpenSpec debe copiar EXACTO el de la spec destino (o `archive` falla).

## Riesgos y notas
- Sin migraciones en esta feature.
- Agregado latency/expiry con etiqueta propia → nivel `warning` en valet.
- Exposición de episodios de señal en `GET /api/breaker` como follow-up.
- Producción: **v0.15.3** (`ghcr.io/atareao/watchbeat:latest`).
