# ADR-0002 · `indeterminate` como estado de primera clase

- **Estado**: aceptado
- **Fecha**: 2026-11-14
- **Contexto**: PLAN-002, Fase 3

## Contexto

Cuando un fallo no es atribuible al target, hay que decidir cómo representarlo. Opciones:
(a) ocultarlo (no registrar), (b) mapearlo a `error`, (c) un estado propio.

## Decisión

Introducir **`indeterminate`** como un estado de primera clase, persistido en
`checks.status`, visible en la UI y en la status page pública, y con semántica propia.

## Razones

- **Ocultarlo** perdería observabilidad: el operador no sabría que hubo un problema de red.
- Mapearlo a `error` lo confundiría con un fallo del lado nuestro ya conocido y seguiría
  contando en el uptime.
- Un estado propio comunica con precisión "**no atribuible**", ni "caído" ni "oculto".

## Semántica

- **No** dispara notificación de caída, **no** cambia `was_up`, **no** cuenta como
  recuperación.
- **No** toca `failed_checks`/`confirmations_required`.
- Se **excluye** del uptime (numerador y denominador).
- Un corte sostenido emite **un** evento SSE (en la transición), no uno por check.
- En el timeline, un bucket sin checks atribuibles emite `dominant_status="indeterminate"`.

## Consecuencias

- Todos los puntos de render (dashboard, detalle, status page) y las queries de uptime
  deben contemplarlo.
- El frontend lo distingue por **doble canal** (color violeta `#722ed1` + icono), no solo
  por color (accesibilidad).
