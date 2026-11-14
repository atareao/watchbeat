# Design

## Context

Ver `proposal.md` — Why. Tras la Fase 1:
- `dns::resolve` devuelve `DnsError::{Timeout,Servfail,Nxdomain,Other}`, pero en
  los checkers ese error se aplana a un `String` y se pierde la tipología.
- `checker/mod.rs:158` clasifica groseramente: `if e.is_timeout() { "down" } else { "error" }`.
- La tabla `checks` no tiene dónde guardar la causa.

## Goals / Non-Goals

**Goals**
- `FailureCause` determinista y tipado, derivado del error real.
- Persistir la causa (`checks.error_kind`).
- Clasificar en HTTP, TCP y TLS; exponer `error_kind` en la API.

**Non-Goals**
- Cambiar la semántica de estado (Fase 3).
- Reintentos.
- Clasificar `ping`/`heartbeat` (se quedan con `cause = None`).
- Mostrarlo en la UI (fase posterior si procede).

## Decisions

- **D1 · `FailureCause` como enum.** Variantes y serialización `snake_case`:
  `DnsNxdomain`, `DnsTimeout`, `DnsServfail`, `ConnectRefused`, `ConnectTimeout`,
  `NetworkUnreachable`, `TlsError`, `HttpStatus`, `BodyMismatch`, `Other`.
- **D2 · Clasificación DNS determinista.** En lugar de introspeccionar la cadena
  de errores de reqwest (frágil entre versiones), `HttpChecker` pre-resuelve el
  host con `dns::resolve` cuando el target no es IP literal. Es barato (la caché
  de la Fase 1 evita el round-trip habitual) y mapea `DnsError` → `FailureCause`
  de forma exacta. TCP/TLS ya resuelven con `dns::resolve`: sólo hay que conservar
  el tipo en vez de aplanarlo.
- **D3 · Mapeo error→causa.** `io::ErrorKind::ConnectionRefused` → `ConnectRefused`;
  `NetworkUnreachable`/`HostUnreachable` → `NetworkUnreachable`; timeout de
  conexión → `ConnectTimeout`; errores de rustls/handshake → `TlsError`; status
  HTTP inesperado → `HttpStatus`; `expected_body` no cumple → `BodyMismatch`;
  resto → `Other`.
- **D4 · Persistencia aditiva.** `error_kind TEXT` nullable; sólo se escribe en
  fallos. Serialización del enum `snake_case` vía `serde`. Migración idempotente
  (patrón `ALTER TABLE` ya usado en `db.rs`).
- **D5 · API aditiva.** `CheckResult` añade `error_kind: Option<String>`
  (`#[serde(skip_serializing_if = "Option::is_none")]`). No rompe consumidores.
- **D6 · Mapeo único causa→status (enmienda).** El status observable se deriva de
  una **única** función `status_for(cause)` compartida por HTTP, TCP y TLS, para
  que la misma causa no produzca status distintos según el tipo de monitor:

  | Causa | Status |
  |---|---|
  | `dns_nxdomain`, `connect_refused`, `tls_error`, `http_status`, `body_mismatch` | `down` (target culpable) |
  | `dns_timeout`, `dns_servfail`, `connect_timeout`, `network_unreachable`, `other` | `error` (no atribuible / lado nuestro) |

  Esta fase **elimina incoherencias legacy** de estado (p. ej. un timeout de
  conexión pasa de `down` a `error`). La atribución fina con salud de red
  (estado `indeterminate`) es la Fase 3.

## Risks / Trade-offs

- [El pre-resolve en HTTP añade una consulta] → la caché la absorbe; sólo un host
  frío paga el round-trip (que de todos modos pagaría reqwest).
- [Cadenas de error de reqwest variables entre versiones] → por eso D2 evita
  inspeccionarlas para el caso DNS; el resto usa `ErrorKind`/tipos estables.
- [Nuevos valores en `error_kind`] → sin impacto en la UI en esta fase.

## Migration Plan

- `ALTER TABLE checks ADD COLUMN error_kind TEXT`, idempotente, al inicializar.
- Sin backfill (los checks antiguos quedan con `NULL`). Reversible en la práctica
  (columna ignorada si se revierte el código).

## Open Questions

- Ninguna que cambie las specs.
