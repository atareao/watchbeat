# Proposal

**Issue:** [#70](https://github.com/atareao/watchbeat/issues/70)
**Plan:** `plans/PLAN-013.md`

## Why

El evento SSE `type:"check"` (`backend/src/scheduler.rs:577-589`) no incluye `error_kind`, aunque el check ya lleva la causa (`CheckResult.error_kind`, `scheduler.rs:528`) y la API REST ya la expone. Como el payload del evento tampoco la trae, `MonitorDetail` se ve obligado a un refetch extra (`fetchChecks(id, 1, 1)` en `reloadAll`, `frontend/src/pages/MonitorDetail.tsx:894`) solo para rellenar `latestCheck.error_kind`, y hoy fija `error_kind: null` en el update optimista (`:918-920`). Añadir `error_kind` al evento elimina ese refetch y hace el panel en tiempo real autocontenido. Es el bloque `c` de #62.

## What Changes

- El payload SSE `type:"check"` incluye una clave nueva `error_kind` con el wire value de `FailureCause` (snake_case) o `null` si no hay causa atribuible. Cambio **aditivo**.
- La construcción del payload se extrae a una función pura testeable (`check_event_payload`) para poder fijar el contrato con un test unitario.
- Frontend: `CheckEvent` (`hooks/useSse.ts`) gana `error_kind: string | null`; `MonitorDetail` lo consume desde el evento y deja de refetchear el último check en el camino SSE.

## Capabilities

### Modified Capabilities

- `scheduler`: el requirement de la guarda de suscriptores SHALL relajarse — en lugar de «payload format unchanged», el payload SHALL incluir `error_kind` (`null` cuando no hay causa), preservando la guarda `receiver_count() > 0`.

## Impact

- `backend/src/scheduler.rs` (construcción del payload + test unitario del contrato).
- `frontend/src/hooks/useSse.ts` (tipo `CheckEvent`).
- `frontend/src/pages/MonitorDetail.tsx` (consumo de `error_kind` + eliminación del refetch innecesario).
- `docs/sse-error-kind.md` (documentación de la feature).
- Sin migración de esquema ni cambio de contrato de API REST (la API ya devuelve `error_kind`).
