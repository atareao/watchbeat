# Handoff — watchbeat — 2026-11-14 (sesión #76)

## Objetivo
Cerrar los follow-ups del Robust Check Engine (#62) y publicar la release acumulada.

## Estado
- **Release v0.16.0 publicada** (`main` + tag `v0.16.0` + GitHub Release).
  - PR #84 (`release/0.16.0` → `main`) mergeado: `02009792`.
  - Bump 0.15.3 → 0.16.0 (`vampus --minor`) + `CHANGELOG.md` curado.
  - `development` re-sincronizado: `5bcd41a3` (1 commit por delante de `main`).
  - Imagen `ghcr.io/atareao/watchbeat` construyéndose por el tag (workflow `docker-multiarch`).
- **#62 y sus follow-ups (#69–#72, #63) cerrados** — 0 issues abiertos.
- Suite: **381 tests backend** verdes; `fmt`/`clippy` limpios; `tsc`+`build` verdes.

### Contenido de v0.16.0
- feat: canario configurable (#69) · `error_kind` en SSE (#70) · estado del breaker en API/dashboard (#71) · agrupación latency/expiry (#72) · timeline accesible + test SNI (#63).
- perf: dedupe DNS single-flight.
- fix: resolución JWKS sin recursión (#67).
- ci: workflow `security` (cargo/npm audit).

## Pendiente
1. Follow-ups de #63 (no bloqueantes): contraste AA de los `Tag` de `MonitorCard`; roving tabindex en el timeline; migrar `statusStyles.ts` a `.tsx`.
2. Nº de plan libre: **PLAN-017**.

## Verificación
```bash
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
cd frontend && npx tsc --noEmit && npm run build
```

## Metodología / reglas
- SDD+TDD: change OpenSpec aprobado por el usuario ANTES de escribir código; luego RED→GREEN→REFACTOR verificado por CLI.
- Gitflow: `feature/*` desde `development`; **PR a `development`**; release `release/*` → **PR a `main`**; nunca commits directos a `main`/`development`.
- Commits con gitmoji + conventional commits.
- Planes en `plans/PLAN-<NNN>.md` (siguiente libre: PLAN-017); al cerrar → renombrar a `-ARCHIVED.md`.
- Docs por feature en `docs/<feature>.md`. El **issue es el punto de entrada** (crear/asignar a `atareao`).
- Header del delta OpenSpec debe copiar EXACTO el de la spec destino (o `archive` falla).
- **Los PRs a `development` NO auto-cierran issues** (`Closes` solo dispara en `main`): cerrarlos a mano.

## Riesgos y notas
- Sin migraciones.
- `npm run lint` no existe en el repo (sin eslint).
- Producción: **v0.16.0** (`ghcr.io/atareao/watchbeat:latest`).
