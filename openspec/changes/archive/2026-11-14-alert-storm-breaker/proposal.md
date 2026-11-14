# Proposal

**Issue:** [#39](https://github.com/atareao/watchbeat/issues/39)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 5)

## Why

Cuando la red del host, un proveedor DNS o un tramo de red intermedio falla, **muchos
monitores caen en el mismo instante**. Hoy cada caída dispara su propia notificación
individual, así que una única incidencia de infraestructura genera **N alertas** (una
tormenta) que saturan al operador y esconden la causa real.

La Fase 3 ya evita los falsos `down` cuando el canario de red está caído (`indeterminate`).
Pero cuando el canario está sano y aun así **varios targets fallan a la vez** (p. ej. el
proveedor DNS del que dependen varios servicios), seguimos mandando N alertas por un solo
suceso. Esta fase añade un **breaker / anti-tormenta**.

## What Changes

- Nuevo módulo `backend/src/breaker.rs` con una máquina de estados **pura y testeable**
  (`StormBreaker`) con reloj inyectado:
  - detecta **tormenta** cuando el número de monitores que caen dentro de una ventana
    de `Y` segundos alcanza `máx(min_monitors, ceil(X % · monitores_habilitados))`;
  - al abrir, emite **una sola alerta agregada** ("monitorización degradada") con el
    recuento y la lista de monitores afectados;
  - mientras está abierto, **suprime** las notificaciones individuales (`down` y `up`);
  - se **rearma** (vuelve a `Closed`) cuando pasa la ventana sin nuevas caídas → dedupe
    por episodio: **una alerta por tormenta**.
- El despacho de notificadores del scheduler (`scheduler.rs`) consulta el breaker antes
  de enviar; la decisión es `Pass` / `Suppress` / `OpenAggregate`.
- **Configuración por settings** (tabla KV) + **nueva pestaña "Tormenta"** en
  `Settings.tsx`: activar/desactivar, umbral `X %`, ventana `Y s`, mínimo de monitores y
  plantilla de la alerta agregada.
- Nueva plantilla por defecto `default_template_storm` y comando
  `SchedulerCommand::ReloadBreaker` para recargar la config en caliente.
- **No cambia** la semántica de estados de `checks`, el canario, la atribución, la API de
  monitores ni el `interval_seconds`.

## Capabilities

### New Capabilities
- `breaker`: detección de tormenta, alerta agregada, dedupe por ventana, supresión y
  configuración.

### Modified Capabilities
_(ninguna — la integración con el despacho se especifica dentro de `breaker`)_

## Impact

- **Código backend**: `backend/src/breaker.rs` (nuevo), `backend/src/scheduler.rs`,
  `backend/src/template.rs`, `backend/src/lib.rs`.
- **Código frontend**: `frontend/src/pages/Settings.tsx`.
- **Config**: claves nuevas en la tabla `settings` (con defaults; sin env obligatoria).
- **Sin migración** de esquema. **Sin** cambios de API incompatibles.
