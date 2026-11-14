# Feature: `error_kind` en el evento SSE

- **Issue:** [#70](https://github.com/atareao/watchbeat/issues/70) (sub-issue bloque `c` de #62)
- **Plan:** `plans/PLAN-013.md`
- **Change OpenSpec:** `sse-error-kind` (archivado en `openspec/specs/scheduler/spec.md`)

## Qué es

El evento SSE `type:"check"` (que empuja el estado de un monitor al panel en
tiempo real) ahora incluye la clave `error_kind` con la causa de fallo
clasificada (`FailureCause`, wire snake_case) o `null` si el check no tiene una
causa atribuible. Antes, el evento no la traía y `MonitorDetail` debía hacer un
`fetchChecks(id, 1, 1)` extra solo para rellenarla.

## Ficheros implicados

- `backend/src/scheduler.rs` — nueva función pura `check_event_payload(monitor, check) -> String`
  (usada en `run_monitor_check`), que añade `"error_kind": check.error_kind` al payload.
  Tests inline (`#[cfg(test)]`) del shape y del payload emitido vía `run_monitor_check`.
- `frontend/src/hooks/useSse.ts` — `CheckEvent` gana `error_kind: string | null`.
- `frontend/src/pages/MonitorDetail.tsx` — consume `event.error_kind` en el update
  optimista de `latestCheck`, elimina el refetch del último check en el camino SSE y
  añade un poll de fallback de 60 s.
- `frontend/src/api/http.ts` — `CheckResult.error_kind?: string | null` (el backend
  omite la clave cuando es `None`).

## Cómo funciona

1. En cada check relevante (`is_first_check || status_changed || latency_breach`) y
   solo si hay suscriptores (`event_tx.receiver_count() > 0`), el scheduler serializa
   el payload con `check_event_payload`, que incluye `error_kind` (la clave está
   **siempre** presente; `null` cuando `check.error_kind` es `None`).
2. `error_kind` es el wire value de `FailureCause` (`connect_refused`,
   `dns_timeout`, `tls_error`, `http_status`, …), idéntico al que ya expone la API
   REST en `GET /api/checks`.
3. El frontend refleja el evento en `latestCheck` (incluido `error_kind`) sin volver a
   pedir el último check.
4. **Fallback poll:** un `setInterval` de 60 s en `MonitorDetail` hace un refresh
   completo (`reloadAll`) — ver *Decisiones*.

## Configuración

Ninguna. Sin variables de entorno ni settings nuevos. Cambio **aditivo** de contrato:
consumidores que ignoren `error_kind` no se rompen.

## Cómo habilitar / desplegar

No requiere pasos: viaja con el build normal (backend embebe el SPA). Sin migración
de esquema ni cambios de API.

## Decisiones

- **Clave siempre presente** (`null` si no hay causa), para un contrato estable y un
  tipo simple en TS (`string | null`).
- **Función pura testeable** (`check_event_payload`) para fijar el shape del payload
  con un test unitario determinista.
- **Se elimina el refetch por evento** en el camino SSE (objetivo del issue: cero
  round-trips por evento).
- **Poll de fallback de 60 s** (opción a de la auditoría): los campos `tls_cert_*`
  **no** viajan en el evento SSE, así que sin el refetch la tarjeta de certificado
  podría congelar un aviso «expira pronto» falso y permanente tras una renovación.
  El poll repone esos campos y además cubre una caída de SSE. Coste aceptado: 4
  req/min con la página abierta.

## Follow-ups

- Refinamientos baratos del poll si molesta: no refrescar con `document.hidden`, o
  saltar el poll si hubo un evento SSE en los últimos ~60 s.
- Alternativa de fondo (opción c): incluir `tls_cert_*` en el payload SSE para poder
  eliminar el poll.
- Housekeeping relacionado: `frontend/dist/index.html` está trackeado pese a
  `dist/` en `.gitignore`; la receta `just check-spec` que cita AGENTS.md no existe
  (la puerta SDD es `openspec validate <change>`).
