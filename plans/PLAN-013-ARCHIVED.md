# PLAN-013 — `error_kind` en el evento SSE (#70)

- **Issue:** [#70](https://github.com/atareao/watchbeat/issues/70) (sub-issue bloque **c** de #62)
- **Change OpenSpec:** `sse-error-kind`
- **Rama:** `feature/sse-error-kind`
- **Estado:** 🟡 Esperando aprobación de la spec (SDD)

## Objetivo

Añadir `error_kind` (taxonomía `FailureCause`, wire snake_case) al payload del evento SSE `type:"check"` para que `MonitorDetail` no tenga que refetchear el último check al recibir una caída. Cambio **aditivo** de contrato.

## Contexto / hallazgos (auditados)

- `FailureCause` — `backend/src/checker/mod.rs:62-108`: enum `Copy`, **solo `Serialize`**, `#[serde(rename_all = "snake_case")]`; `as_str()` devuelve el wire value. 10 variantes: `dns_nxdomain, dns_timeout, dns_servfail, connect_refused, connect_timeout, network_unreachable, tls_error, http_status, body_mismatch, other`.
- La causa ya viaja en el check: `CheckOutcome.cause: Option<FailureCause>` (`checker/mod.rs:227`) → `CheckResult.error_kind: Option<String>` construido en `scheduler.rs:528` (`outcome.cause.map(|c| c.as_str().to_string())`), persistido en `checks.error_kind` (`db.rs:91,257-258`) y ya expuesto por la API REST (`models.rs:38-54`, `routes/checks.rs`, `routes/monitors.rs:386-387`).
- El evento SSE se construye **ad-hoc** con `serde_json::json!` en `scheduler.rs:577-589` y se serializa a `String` (`broadcast::Sender<String>`). Guarda: `event_tx.receiver_count() > 0 && (is_first_check || status_changed || latency_breach)`. Campos actuales: `type, monitor_id, monitor_name, status, response_time_ms, error_message, checked_at`. **No incluye `error_kind`**; `check.error_kind` ya está en el mismo scope.
- OpenSpec: **no existe** spec `sse`/`events`. El contrato SSE vive en `openspec/specs/scheduler/spec.md`. El requirement `### Requirement: El scheduler SHALL mantener la guarda de suscriptores` (L101-108) **fija el payload como invariante** (`payload format unchanged`), así que el delta debe MODIFICAR ese requirement con **header exacto** o `opens,pec archive` fallará.
- Frontend: `CheckEvent` (`frontend/src/hooks/useSse.ts:3-11`) **sin** `error_kind`. `MonitorDetail.tsx:918-920` fija `error_kind: null` en el update optimista y `:925` llama `reloadAll()`, cuyo `fetchChecks(currentId, 1, 1)` (`:894`) solo sirve para rellenar `latestCheck.error_kind`.

## Contrato de wire (delta)

Payload SSE `type:"check"` pasa a:

```json
{
  "type": "check",
  "monitor_id": "…",
  "monitor_name": "…",
  "status": "up|down|error|warning|indeterminate",
  "response_time_ms": 123,
  "error_message": null,
  "error_kind": null,
  "checked_at": "…"
}
```

- `error_kind: string | null` — wire value de `FailureCause` (`dns_nxdomain` … `other`), `null` si el check no tiene causa atribuible.
- Se incluye **siempre** la clave (contrato estable) aunque el valor sea `null`.

## Fases y tareas (TDD: RED → GREEN → REFACTOR)

### Fase 1 — Cambio OpenSpec (hecho, a la espera de aprobación)
- `sse-error-kind`: `proposal.md`, `tasks.md`, `specs/scheduler/spec.md` con `## MODIFIED Requirements` (header exacto copiado de la spec destino).
- **STOP**: esperar aprobación explícita del usuario antes de escribir código/tests.

### Fase 2 — Backend (`@backend-dev`)
- **RED**: test unitario que fija el shape del payload SSE incluyendo `error_kind` (caso con causa → wire value; caso sano → `null`).
- **GREEN**: extraer la construcción del payload a una función pura testeable (`fn check_event_payload(monitor: &Monitor, check: &CheckResult) -> String`, o un struct `#[derive(Serialize)]`) en `scheduler.rs`; añadir "error_kind": check.error_kind al JSON; usar la función en `run_monitor_check` preservando la guarda `receiver_count() > 0` y las condiciones de relevancia.
- **REFACTOR**: `cargo fmt --check`, `cargo clippy -- -D warnings`, suite completa verde.

### Fase 3 — Frontend (`@frontend-dev`)
- `hooks/useSse.ts`: añadir `error_kind: string | null` a `CheckEvent`.
- `MonitorDetail.tsx`: consumir `ev.error_kind` en el update optimista (`:918-920`) en vez de `null`; **eliminar** el refetch `fetchChecks(id, 1, 1)` del camino SSE (introducir un refresh ligero de timeline + tabla) conservando `reloadAll` para los demás usos (montaje/manual).
- `npx tsc --noEmit` + `npm run build`.

### Fase 4 — Doc y cierre
- `docs/sse-error-kind.md`.
- `openspec archive sse-error-kind` (headers casando).
- PR a `development` con `Closes #70` (gitflow + gitmoji).

## Verificación
- Backend: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`.
- Frontend: `npx tsc --noEmit`, `npm run build`.
- SDD: `openspec validate sse-error-kind` (puerta SDD; `just check-spec` no existe como receta).
- Auditoría: `@rust-reviewer` (backend) + `@react-reviewer` (frontend).

## Riesgos / notas
- El header del delta debe copiar **EXACTAMENTE** `### Requirement: El scheduler SHALL mantener la guarda de suscriptores`.
- No romper la guarda de 0 asignaciones (`receiver_count() > 0`).
- `reloadAll` se usa también al montar y tras acciones manuales: el refetch ligero debe aplicarse **solo** al camino SSE.
- Issue: [#70](https://github.com/atareao/watchbeat/issues/70).
- **Frescura TLS:** los campos `tls_cert_*` NO viajan en el evento SSE; al eliminar el refetch por evento, un monitor TLS podría mostrar un aviso de expiración falso hasta el siguiente refresh. Mitigación adoptada: **poll de fallback de 60 s** en `MonitorDetail` (opción a de la auditoría `@react-reviewer`).
- **Coste del poll:** 4 req/min con la página de detalle abierta (monitor + timeline + último check + tabla). Aceptado; refinable con `document.hidden` o «skip si hubo evento SSE reciente».