# Handoff — watchbeat — 2026-11-14 (sesión #78)

## Objetivo
Publicar la release acumulada (v0.17.0) con el cierre de #86.

## Estado
- **Release v0.17.0 publicada** (`main` + tag `v0.17.0` + GitHub Release).
  - PR #92 (`release/0.17.0` → `main`) mergeado: `0528207a`.
  - Bump 0.16.0 → 0.17.0 (`vampus --minor`) + `CHANGELOG.md`.
  - `development` re-sincronizado: `0d63d7ec`.
  - Imagen `ghcr.io/atareao/watchbeat` construyéndose por el tag.
- **#86** (a11y follow-ups) entregado en v0.17.0: contraste AA del `Tag`, roving tabindex, `statusStyles.tsx`.
- **#90 corregido**: el archive de #86 (PR #88) había dejado el change original como "activo" en
  `development`; lo arregló el PR #91 (`d0e44bcf`). `openspec list` → sin changes activos.
- Issues abiertos: **0** · PRs abiertos: **0**.

## Hallazgo importante (bug de tooling)
- **`openspec archive` + git**: al archivar, verificar SIEMPRE que el commit incluye las entradas
  `D`/`M` (borrado del change original + modificación de la spec destino), no solo las `A`. Si no,
  el change queda como "activo" en un clone limpio. Comprobar con `git show <commit> --name-status`.
- **antd 6**: un `color` custom no-preset en un `Tag` cae en `variant:'filled'` → el hex pasa a ser
  **color del texto** y el fondo es un **tinte claro**. Para fondo = hex del estado usar
  **`variant="solid"`**. Verificado por SSR.
- **Los PRs a `development` NO auto-cierran issues** (`Closes` solo dispara en `main`): cerrarlos a mano.

## Pendiente
1. Nº de plan libre: **PLAN-018**.

## Verificación
```bash
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
cd frontend && npx tsc --noEmit && npm run build
```

## Metodología / reglas
- SDD+TDD: change OpenSpec aprobado por el usuario ANTES de escribir código; luego RED→GREEN→REFACTOR verificado por CLI.
- Gitflow: `feature/*` desde `development`; **PR a `development`**; release `release/*` → **PR a `main`**; nunca commits directos a `main`/`development`.
- Commits con gitmoji + conventional commits.
- Planes en `plans/PLAN-<NNN>.md` (siguiente libre: PLAN-018); al cerrar → renombrar a `-ARCHIVED.md`.
- Docs por feature en `docs/<feature>.md`. El **issue es el punto de entrada** (crear/asignar a `atareao`).
- Header del delta OpenSpec debe copiar EXACTO el de la spec destino; un bloque MODIFIED reemplaza el requirement completo (conservar TODOS sus escenarios).

## Riesgos y notas
- Sin migraciones.
- `npm run lint` no existe en el repo (sin eslint). No hay vitest.
- `openspec validate --all --strict` reporta 8 specs pre-existentes con `Purpose` placeholder (no relacionadas).
- Producción: **v0.17.0** (`ghcr.io/atareao/watchbeat:latest`).
