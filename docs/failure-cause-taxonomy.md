# Failure cause taxonomy (`error_kind`)

## Qué es

Cuando un check falla, WatchBeat ya no se limita a guardar `down`/`error`: además
etiqueta **por qué** falló con un valor tipado (`FailureCause`). Esa causa se
persiste en `checks.error_kind` y se expone en la API de checks (cambio aditivo).

- Issue: [#36](https://github.com/atareao/watchbeat/issues/36)
- Plan: `plans/PLAN-002.md` — Robust Check Engine, Fase 2
- Change proposal: `openspec/changes/failure-cause-taxonomy/`

## Ficheros implicados

| Fichero | Papel |
|---|---|
| `backend/src/checker/mod.rs` | `FailureCause`, `CheckOutcome.cause`, `HttpChecker`, `TcpChecker`, mapeo error→causa |
| `backend/src/checker/tls.rs` | `TlsChecker`: distingue connect / handshake y conserva el `DnsError` |
| `backend/src/models.rs` | `CheckResult.error_kind` (serde aditivo) |
| `backend/src/db.rs` | Columna `checks.error_kind`, migración idempotente, persistencia y lectura |
| `backend/src/scheduler.rs`, `backend/src/routes/monitors.rs` | Reenvían `outcome.cause` a la fila persistida |
| `backend/tests/db_integration.rs` | Migración idempotente, round-trip de `error_kind` |

## Cómo funciona

**Taxonomía** (wire contract `snake_case`, idéntico a `FailureCause::as_str()` y a
lo que se guarda en DB):

```
dns_nxdomain · dns_timeout · dns_servfail · connect_refused · connect_timeout
network_unreachable · tls_error · http_status · body_mismatch · other
```

**Clasificación**

- **HTTP** (`checker/mod.rs`): el host se pre-resuelve con `dns::resolve` cuando el
  target **no** es una IP literal. Así un fallo de DNS se clasifica desde el
  `DnsError` tipado en vez de adivinar a partir de las cadenas de error de reqwest
  (que cambian entre versiones). La consulta sale casi gratis por la caché TTL del
  resolvedor de la Fase 1. Después de la petición: status inesperado →
  `http_status`; `expected_body` no cumple → `body_mismatch`; error de transporte →
  cadena de errores hasta el `io::Error` → `connect_refused`,
  `network_unreachable`, `connect_timeout`; HTTPS sin socket error → `tls_error`.
- **TCP** (`TcpChecker`): conserva el tipo `DnsError` (no lo aplana a `String`) y
  clasifica el fallo de conexión por `io::ErrorKind`/`errno`.
- **TLS** (`TlsChecker`): separa `Connect` de `Handshake`; un handshake fallido es
  `tls_error`, un connect fallido se clasifica como en TCP.
- **ping / heartbeat**: fuera de la taxonomía → `cause = None`.

**Semántica de estado (unificada)**: el status observable se deriva de una única
función `status_for(cause)` compartida por HTTP, TCP y TLS, para que la misma
causa no dé status distintos según el monitor:

| Causa | Status |
|---|---|
| `dns_nxdomain`, `connect_refused`, `tls_error`, `http_status`, `body_mismatch` | `down` (target culpable) |
| `dns_timeout`, `dns_servfail`, `connect_timeout`, `network_unreachable`, `other` | `error` (no atribuible / lado nuestro) |

Esto **elimina incoherencias legacy** (p. ej. timeout de conexión pasa de `down` a
`error`). La atribución fina con salud de red (`indeterminate`) es la Fase 3.

## Variables / configuración

Ninguna nueva. La migración es automática al abrir la DB.

## Cómo habilitar / desplegar

Transparente: al arrancar, `Database::open` añade `checks.error_kind` si falta
(`ALTER TABLE ... ADD COLUMN`, idempotente). Sin backfill: los checks antiguos
quedan con `error_kind = NULL`. Revertir el código es inocuo: la columna se
ignora.

## Decisiones

- **D2** del `design.md`: pre-resolver en HTTP en lugar de introspeccionar las
  cadenas de error de reqwest.
- **`#[serde(skip_serializing_if = "Option::is_none")]`** en `CheckResult.error_kind`:
  los consumidores del payload anterior no ven ningún campo nuevo. `serde(default)`
  permite seguir deserializando payloads sin el campo.
- **`FailureCause` es `Copy`**, para poder reenviarlo a la fila persistida sin
  clonar el `CheckOutcome`.
- Una conexión rechazada es `down` (el host responde, el puerto está cerrado ⇒
  target culpable). Ver la tabla unificada causa→status más arriba.

## Follow-ups

- Fuera de alcance (heredado de la Fase 1): `rustls` recibe `ring` y `aws-lc-rs`,
  así que `ClientConfig::builder()` exige proveedor explícito. En tests se resuelve
  con `install_crypto_provider()`; en despliegue conviene fijar el proveedor (issue
  aparte).
- Fase 3 (#37): atribución de red, canario y estado `indeterminate`.
