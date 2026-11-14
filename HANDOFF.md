# Handoff — watchbeat — 2026-11-14 20:38

## Objetivo
Cerrar los follow-ups del Robust Check Engine (#62) uno por sesión, con SDD+TDD, dejando cada feature en un PR a `development`.

## Estado
- Hecho:
  - **#69** canario de red configurable — PR #74 mergeado.
  - **#70** `error_kind` en el evento SSE — PR #75 mergeado (`71bc2c4d`).
  - **#71** estado del storm breaker (`closed`/`open`) en API y dashboard — PR #76 mergeado (`562ccff9`): `GET /api/breaker` (protegido), breaker compartido vía `SchedulerManager::breaker()`, badge en Dashboard + indicador «Suprimido» en `MonitorCard`.
  - Artefactos: `docs/sse-error-kind.md`, `docs/breaker-state.md`; `plans/PLAN-013-ARCHIVED.md`, `plans/PLAN-014-ARCHIVED.md`; changes archivados en `openspec/changes/archive/2026-11-14-{sse-error-kind,breaker-state}/`; specs fusionadas en `openspec/specs/{scheduler,breaker}/spec.md`.
  - Suite: 362 tests backend verdes; `tsc`+build frontend OK.
- En curso: nada a medias. #70 y #71 cerrados y mergeados.
- Rama actual: `development` (base: `development`). PRs abiertos: ninguno. Sin ramas `feature/*` abiertas.
- Working tree: 5 entradas «borradas» sin commitear en el índice (blobs vacíos obsoletos de `openspec new change`, **no están en HEAD**): `openspec/changes/breaker-state/{.openspec.yaml,proposal.md,tasks.md,specs/breaker/spec.md}` y `plans/PLAN-014.md`. El contenido real vive en `openspec/changes/archive/2026-11-14-breaker-state/` y `plans/PLAN-014-ARCHIVED.md`.

## Pendiente
Abordar **#72** (📬 agrupar notificaciones latency/expiry en el breaker) en sesión nueva: issue → change OpenSpec → TDD → PR a `development` con `Closes #72`; después queda **#63** (♿ a11y del timeline + test SNI real).

## Punto de partida
```bash
cd /data/rust/watchbeat
git checkout development && git pull --ff-only
git status                                   # limpiar los 5 borrados obsoletos (ver Riesgos)
openspec list                                # debe salir: "No active changes found."
git rm --cached openspec/changes/breaker-state/* plans/PLAN-014.md   # opcional: limpiar índice
openspec new change <nombre>                 # p.ej. latency-expiry-aggregation
just gf-feature <nombre>                     # rama feature/<nombre> desde development
# verificación
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
cd ../frontend && npx tsc --noEmit && npm run build
openspec validate <nombre> --strict
```
Nota: `just check-spec` NO existe; la puerta SDD es `openspec validate <change> --strict`.

## Metodología / reglas
- SDD+TDD: change OpenSpec aprobado por el usuario ANTES de escribir código; luego RED→GREEN→REFACTOR verificado por CLI.
- Gitflow: `feature/*` desde `development`; **PR a `development`**; nunca commits directos a `main`/`development`.
- Commits con gitmoji + conventional commits.
- Planes en `plans/PLAN-<NNN>.md` (**siguiente libre: PLAN-015**); al cerrar → renombrar a `-ARCHIVED.md`.
- Docs por feature en `docs/<feature>.md`. El **issue es el punto de entrada** (crear/asignar a `atareao`).
- Delegar escritura de código a subagentes; auditar con `@rust-reviewer` / `@react-reviewer` antes de cerrar.
- Header del delta OpenSpec debe copiar EXACTO el de la spec destino (o `archive` falla); un MODIFIED no puede eliminar escenarios existentes.

## Riesgos y notas
- **Índice git con 5 entradas obsoletas** (ver Working tree): `git rm --cached` las limpia (no están en HEAD). Confirmar con `git ls-tree HEAD openspec/changes/`.
- **Rearme perezoso del breaker**: `state:"open"` puede quedar vigente sin transiciones (sólo reevalúa en `on_down`/`on_up`). Documentado en `docs/breaker-state.md`; follow-up: `snapshot(now)`.
- **Poll de 60 s en `MonitorDetail`** (#70): 4 req/min con la página abierta; refinable con `document.hidden` o salto si hubo evento SSE reciente. Alternativa de fondo: llevar `tls_cert_*` al payload SSE.
- **Housekeeping**: `frontend/dist/index.html` trackeado pese a `dist/` en `.gitignore` (Docker lo reconstruye; un `cargo build` local embebería un SPA antiguo) → valorar destrackear. Doble lockfile `package-lock.json` + `pnpm-lock.yaml`. `libsqlite3-sys` 0.37 (RUSTSEC-2026-0459), a la espera de que sqlx suba. CHANGELOG autogenerado desactualizado.
- Producción: **v0.15.3** (`ghcr.io/atareao/watchbeat:latest`, rev `147a7bcf`), `healthy`, 7 monitores `up` (despliegue Dockge).
- Esta sesión cerró **dos** features (#70 y #71); por higiene (§V) abrir **sesión nueva** para #72.
