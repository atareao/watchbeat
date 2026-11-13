# Proposal

**Issue:** [#36](https://github.com/atareao/watchbeat/issues/36)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 2)

## Why

Hoy, cuando un check falla, se guarda como `error` o `down` **sin decir por qué**.
No se distingue "no se resolvió el nombre", "conexión rechazada", "timeout de
conexión", "error TLS" o "HTTP 500". Sin esa causa no se puede atribuir el fallo
(objetivo de la Fase 3) ni diagnosticar desde la UI. La Fase 1 ya produce un
`DnsError` tipado que hoy se pierde al no tener dónde mapearse.

## What Changes

- **Nueva capacidad `checker`**: `FailureCause` clasifica el motivo del fallo, de
  forma determinista, a partir del error real (sin repetir la petición).
- `CheckOutcome` incorpora `cause: Option<FailureCause>`; los checkers HTTP, TCP y
  TLS lo rellenan.
- **Capacidad `db` modificada**: nueva columna `checks.error_kind` (migración
  aditiva) para persistir la causa.
- La API de checks devuelve `error_kind` (cambio **aditivo**, no breaking).

**Nota de estado**: esta fase **unifica** el mapeo causa→status entre los tres
checkers (misma causa ⇒ mismo status), lo que puede cambiar el status observable
de algunos fallos legacy (p. ej. timeout de conexión: `down` → `error`). El estado
`indeterminate` y la atribución por salud de red quedan **fuera de alcance**
(Fase 3).

## Capabilities

### New Capabilities
- `checker`: taxonomía y clasificación de la causa de fallo de un check.

### Modified Capabilities
- `db`: nueva columna `error_kind` en `checks` y su persistencia.

## Impact

- **Código**: `backend/src/checker/mod.rs`, `backend/src/checker/tls.rs`,
  `backend/src/models.rs`, `backend/src/db.rs`, `backend/src/routes/checks.rs`.
- **DB**: `ALTER TABLE checks ADD COLUMN error_kind TEXT` (aditiva, idempotente).
- **Sin** cambios de frontend (el campo queda disponible en la API).
