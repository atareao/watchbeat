# Proposal

**Issue:** [#37](https://github.com/atareao/watchbeat/issues/37)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 3)

## Why

Aunque la Fase 1 ya cachea el DNS y la Fase 2 etiqueta la causa del fallo, un
fallo por **infraestructura del host** (caída de red, DNS de salida degradado)
sigue registrándose como `down`/`error` y **alerta como si el servicio estuviera
caído**. Falta la pieza que decide *a quién* pertenece el fallo: al **target** o a
**nuestra propia conectividad**.

## What Changes

- **Nueva capacidad `netcheck`**: canario de salud de la propia salida
  (`NetworkHealth`), con sondeos **por IP** (sin depender del DNS del sistema) más
  una resolución DNS de nombre conocido vía el resolver propio.
  Estados: `unknown` / `healthy` / `dns_degraded` / `down`.
- **Nueva capacidad `attribution`**: clasificador determinista
  `classify(cause, health) -> status` con tabla de verdad explícita. Introduce el
  estado **`indeterminate`** (no atribuible al target).
- **`scheduler` modificada**: la atribución se aplica en el instante del check;
  `indeterminate` **no** es una caída alertable (no notifica ni cambia el estado de
  notificación del monitor).
- **`db` modificada**: el cálculo de uptime **excluye** los checks `indeterminate`.
- **Frontend**: nuevo estado visible (badge + timeline + tipo SSE) distinto de `down`.

**No hay migración de DB**: `checks.status` es texto libre; `indeterminate` es un
valor nuevo. Tampoco cambian las causas (`FailureCause`) de la Fase 2.

## Capabilities

### New Capabilities
- `netcheck`: canario de salud de red y su publicación compartida.
- `attribution`: clasificación determinista causa+salud → estado (incl. `indeterminate`).

### Modified Capabilities
- `scheduler`: integración de la atribución y semántica de notificación.
- `db`: el uptime excluye los checks `indeterminate`.

## Impact

- **Código backend**: `backend/src/netcheck.rs` (nuevo), `backend/src/attribution.rs`
  (nuevo), `backend/src/scheduler.rs`, `backend/src/main.rs`, `backend/src/lib.rs`,
  `backend/src/routes/monitors.rs` (run-check).
- **Código frontend**: tipos de estado, badge/timeline y `useSse`.
- **Sin** cambio de esquema; **sin** nuevas variables obligatorias (intervalo del
  canario con default, override opcional).
