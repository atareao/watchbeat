# Proposal

## Why

Al cerrar #63 quedaron tres follow-ups no bloqueantes:

1. Los `Tag` de estado de `MonitorCard` usan **texto blanco** sobre colores que no llegan a AA
   4.5:1 (`#f97316` ≈ 2.8:1, `#22c55e` ≈ 2.3:1): ilegibles para baja visión.
2. El timeline pone `tabIndex={0}` en **cada** barra: una vista de 24 h supone ~150–300 paradas de
   `Tab`, lo que hace el teclado inutilizable en la práctica.
3. El token `statusStyles.ts` construye sus iconos con `createElement` en vez de JSX.

## What Changes

- **Contraste AA**: el texto sobre superficies de estado (`Tag` de `MonitorCard` y equivalentes)
  SHALL alcanzar contraste ≥ 4.5:1 contra su fondo, en tema claro y oscuro.
- **Roving tabindex**: el timeline SHALL exponer **una única parada de `Tab` por gráfico**; las
  flechas ←/→ (y Home/End) mueven la barra activa.
- **`.tsx`**: `statusStyles.ts` → `statusStyles.tsx` usando JSX (refactor sin cambio de
  comportamiento observable).

## Capabilities

### New Capabilities

- `frontend-status-contrast`: contraste accesible de la paleta de estados en las superficies que la
  usan (tags, tarjetas, leyenda), en claro y oscuro.

### Modified Capabilities

- `frontend-timeline-a11y`: el modelo de navegación por teclado pasa de "cada barra es una parada
  de `Tab`" a **roving tabindex** (una parada por gráfico + flechas).

## Impact

- **Frontend**: `frontend/src/theme/statusStyles.ts(x)`, `frontend/src/components/MonitorCard.tsx`,
  `frontend/src/pages/MonitorDetail.tsx` (`HealthLatencyChart`, `HeartbeatChart`),
  `frontend/src/global.css`.
- **Sin cambios** de API, esquema ni backend.
