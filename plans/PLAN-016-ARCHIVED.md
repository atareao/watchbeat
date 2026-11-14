# PLAN-016 — ♿ a11y del timeline + test SNI real

**Issue:** [#63](https://github.com/atareao/watchbeat/issues/63) (sub-issue `f` de #62)
**Change OpenSpec:** `openspec/changes/timeline-a11y-sni-test/`
**Estado:** COMPLETADO — PR #80 mergeado en `development` (`3b10976d`). Change archivado.

## Objetivo

Hacer accesible el timeline de checks (distinguible sin color, con texto accesible y navegable por
teclado) y blindar con un test determinista que el checker TLS envía el hostname correcto como SNI.

## Diseño

- `frontend/src/theme/statusStyles.ts`: token único `estado → { color, pattern, label, icon }`.
- Barras del timeline: patrón no cromático + `role="img"` + `tabIndex` + `aria-label` + tooltip en
  foco.
- Backend: `rcgen` (dev-dep) + servidor TLS local con `ResolvesServerCert` que captura el SNI.
- Sin migraciones; sin cambios de API.

## Tareas

Ver checklist TDD en `openspec/changes/timeline-a11y-sni-test/tasks.md`.

## Verificación

- `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`.
- `npx tsc --noEmit` y `npm run build` (frontend).
- `openspec validate timeline-a11y-sni-test --strict`.
- PR a `development` con `Closes #63`.

## Cierre

- `docs/timeline-a11y-sni-test.md`.
- Archivar change y renombrar este plan a `PLAN-016-ARCHIVED.md`.
