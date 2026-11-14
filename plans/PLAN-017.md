# PLAN-017 — ♿ Follow-ups de a11y del timeline (contraste AA, roving tabindex, .tsx)

**Issue:** [#86](https://github.com/atareao/watchbeat/issues/86) (follow-ups de #63)
**Change OpenSpec:** `openspec/changes/timeline-a11y-followups/`
**Estado:** EN CURSO — pendiente de aprobación de la spec (Fase 1 SDD)

## Objetivo

Cerrar los follow-ups no bloqueantes de #63: contraste AA del texto de estado, roving tabindex en el
timeline (una parada de Tab por gráfico) y migración del token a `.tsx`.

## Diseño

- Token `statusStyles.tsx`: color de texto accesible por estado para los `Tag` (≥ 4.5:1).
- Timeline: `activeIndex` + `onKeyDown` (←/→/Home/End); `tabIndex` roving.
- `.tsx` con JSX; sin cambio de comportamiento.

## Tareas

Ver checklist en `openspec/changes/timeline-a11y-followups/tasks.md`.

## Verificación

- `npx tsc --noEmit` y `npm run build` (frontend).
- Contraste ≥ 4.5:1 verificado (claro/oscuro) y navegación por teclado manual.
- `openspec validate timeline-a11y-followups --strict`.
- PR a `development` con `Closes #86`.

## Cierre

- `docs/timeline-a11y-followups.md`.
- Archivar change y renombrar este plan a `PLAN-017-ARCHIVED.md`.
