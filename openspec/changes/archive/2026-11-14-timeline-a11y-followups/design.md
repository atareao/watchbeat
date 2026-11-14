# Design

## Context

Ver `proposal.md` — Why. Estado actual relevante (tras #63):

- `frontend/src/theme/statusStyles.ts` (143 líneas) exporta `STATUS_STYLES` y `statusStyle()`; los
  iconos se construyen con `createElement` + `decorativeIcon()`.
- `MonitorCard.tsx:163` → `<Tag color={cfg.color}>{statusLabel}</Tag>` (texto blanco sobre el hex
  del estado).
- `MonitorDetail.tsx`: `HealthLatencyChart` (~683) y `HeartbeatChart` (~528) ponen `tabIndex={0}` en
  **cada** barra (hasta `MAX_BARS`), con `role="img"` + `aria-label` + `Tooltip trigger=[hover,focus]`.
- **Sin test runner de frontend** (no vitest): la verificación es `tsc --noEmit` + build + revisión
  manual.

## Goals / Non-Goals

**Goals**: contraste AA del texto de estado en claro y oscuro; una sola parada de Tab por gráfico con
navegación por flechas; `.tsx` idiomático.

**Non-Goals**: cambiar la paleta de las **barras** del timeline (solo el texto sobre superficies de
estado); introducir un test runner de frontend; tocar API/backend/esquema.

## Decisions

### D1 — Contraste del `Tag`: elegir color de texto accesible por estado
En vez de cambiar la paleta global (afectaría al timeline), se elige por estado un **color de texto**
(blanco o casi-negro) que maximice el contraste con el fondo del `Tag`; si ningún texto alcanza
4.5:1, se oscurece ligeramente **solo el fondo del `Tag`**. Se expone un `tagTextColor` (o un par
`tagBg`/`tagText`) en el token. Alternativas consideradas: (a) usar presets de antd
(`color="success"`) — pierde el hex exacto del estado; (b) dejar texto blanco — incumple AA.
Se prefiere D1 por conservar la identidad de color y cumplir AA.

### D2 — Roving tabindex con estado `activeIndex` por gráfico
Cada gráfico mantiene un `activeIndex` (React state). Solo la barra activa lleva `tabIndex={0}`; el
resto `tabIndex={-1}`. `onKeyDown` en el contenedor maneja ←/→/Home/End (con `preventDefault`) y
actualiza `activeIndex`, moviendo el foco a la barra correspondiente (`ref`). Alternativa: `tabIndex`
positivo (rechazada: anti-patrón a11y). Se usa una parada por gráfico, no `role="listbox"` completo,
para minimizar cambios.

### D3 — `statusStyles.ts` → `.tsx`
Renombrar y usar JSX para los iconos (`<CheckCircleOutlined aria-hidden style={{...}} />`), lo que
permite eliminar `decorativeIcon`/`createElement`. Sin cambio de comportamiento.

## Risks / Trade-offs

- [El contraste se valida a mano, no por test] → documentar los ratios calculados en la doc de
  feature y revisar en claro/oscuro con las herramientas de devtools.
- [Roving tabindex puede romper el orden de foco si `activeIndex` queda fuera de rango al filtrar] →
  recortar `activeIndex` al rango de barras visibles en cada render.
- [Renombrar a `.tsx` toca imports] → los imports no incluyen extensión, así que no hay cambios en
  consumidores.

## Migration Plan

Sin migraciones. Rollback = revertir el PR.

## Open Questions

Ninguna que cambie las specs, el enfoque o el desglose de tareas.
