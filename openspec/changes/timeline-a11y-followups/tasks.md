# Tasks

> Nota: no hay test runner de frontend (no vitest). La verificación es `tsc --noEmit` + `npm run build`
> + revisión manual (contraste y teclado). Los cambios no tocan backend.

## 1. Contraste AA

- [x] 1.1 Añadir al token (`statusStyles.tsx`) el color de texto accesible por estado (y, si hace
      falta, un fondo de `Tag` oscurecido) para alcanzar ≥ 4.5:1.
- [x] 1.2 Aplicar en `MonitorCard.tsx` (`<Tag>`): fondo + color de texto del token.
- [x] 1.3 Calcular y anotar los ratios de contraste (claro y oscuro) para la doc de feature.

## 2. Roving tabindex

- [x] 2.1 `HealthLatencyChart`: `activeIndex` + `onKeyDown` (←/→/Home/End) + `tabIndex={0}` solo en
      la barra activa, `-1` en el resto.
- [x] 2.2 `HeartbeatChart`: mismo modelo.
- [x] 2.3 Recortar `activeIndex` al rango de barras visibles al cambiar los datos/filtros.

## 3. statusStyles.tsx

- [x] 3.1 Renombrar `statusStyles.ts` → `statusStyles.tsx` y usar JSX; eliminar `createElement`/
      `decorativeIcon`.

## 4. Verificación y cierre

- [x] 4.1 `npx tsc --noEmit` y `npm run build` verdes.
- [x] 4.2 `@react-reviewer` sobre el diff.
- [x] 4.3 `docs/timeline-a11y-followups.md`.
- [ ] 4.4 PR a `development` con `Closes #86`.
- [ ] 4.5 Archivar change y renombrar `plans/PLAN-017.md` → `-ARCHIVED.md`.
