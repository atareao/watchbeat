# Handoff — watchbeat — 2026-11-14 (sesión #77)

## Objetivo
Cerrar los follow-ups de a11y del timeline (#86) con SDD+TDD y PR a `development`.

## Estado
- **#86 completado y mergeado**: PR #87 (`feature/timeline-a11y-followups`) → `development` (`21ede7c5`).
  - Change `timeline-a11y-followups` **archivado** (PR #88, `fe37866f`) → `openspec/changes/archive/2026-11-14-timeline-a11y-followups/`.
  - Specs: `frontend-timeline-a11y` (modificada: roving tabindex), `frontend-status-contrast` (nueva).
  - `plans/PLAN-017.md` → `plans/PLAN-017-ARCHIVED.md`.
- **Entregado**:
  - Contraste AA del `Tag` de `MonitorCard`: `variant="solid"` + `tagBg`/`tagText` del token (≥4.5:1 en claro y oscuro).
  - Roving tabindex en `HeartbeatChart`/`HealthLatencyChart`: 1 parada de `Tab` por gráfico; ←/→/Home/End; reset al cambiar rango; clamp si la serie se acorta.
  - `statusStyles.ts` → `statusStyles.tsx` (JSX; sin `createElement`/`decorativeIcon`).
- Verificación: `tsc --noEmit` + `npm run build` verdes; `openspec validate` OK para las specs tocadas.
- **Issues abiertos: 0** · PRs abiertos: 0. Rama `development` limpia.
- Producción: **v0.16.0** (release previa). Esta feature aún **no** está en `main` (pendiente de próxima release).

## Hallazgo importante (aprendido en esta sesión)
- **antd 6**: un `color` custom no-preset en un `Tag` cae en `variant:'filled'` → antd usa el hex como
  **color del texto** y pinta un **tinte claro** de fondo (`hsl.l=0.95`). Para que el fondo sea el hex
  del estado hay que usar **`variant="solid"`**. Verificado por SSR del `Tag` real.
- **Los PRs a `development` NO auto-cierran issues** (`Closes` solo dispara en la rama por defecto `main`): cerrarlos a mano.

## Pendiente
1. Nº de plan libre: **PLAN-018**.
2. (Opcional) Próxima release incluirá #86 (`feat`) → bump minor (v0.17.0) si no hay más cambios.

## Verificación
```bash
cd frontend && npx tsc --noEmit && npm run build
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

## Metodología / reglas
- SDD+TDD: change OpenSpec aprobado por el usuario ANTES de escribir código; luego RED→GREEN→REFACTOR verificado por CLI.
- Gitflow: `feature/*` desde `development`; **PR a `development`**; release `release/*` → **PR a `main`**; nunca commits directos a `main`/`development`.
- Commits con gitmoji + conventional commits.
- Planes en `plans/PLAN-<NNN>.md` (siguiente libre: PLAN-018); al cerrar → renombrar a `-ARCHIVED.md`.
- Docs por feature en `docs/<feature>.md`. El **issue es el punto de entrada** (crear/asignar a `atareao`).
- Header del delta OpenSpec debe copiar EXACTO el de la spec destino (o `archive` falla). Un bloque MODIFIED reemplaza el requirement completo → conservar TODOS sus escenarios originales.

## Riesgos y notas
- Sin migraciones. Sin backend tocado en esta feature.
- `npm run lint` no existe en el repo (sin eslint). No hay vitest.
- `openspec validate --all --strict` reporta 8 specs pre-existentes con `Purpose` placeholder (no relacionadas; mismo recuento antes/después de esta sesión).
