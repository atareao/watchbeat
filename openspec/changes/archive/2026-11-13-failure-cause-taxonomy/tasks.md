To check an item, replace `[ ]` with `[x]`.

## Preparación
- [x] Issue #36 creado
- [x] Change proposal OpenSpec creado
- [x] Rama `feature/failure-cause-taxonomy`

## RED — Tests
- [x] Test unitario: clasificación DNS por variante (`dns_nxdomain`, `dns_timeout`, `dns_servfail`)
- [x] Test unitario: `connect_refused`
- [x] Test unitario: `connect_timeout`
- [x] Test unitario: `network_unreachable`
- [x] Test unitario: `tls_error`
- [x] Test unitario: `http_status` (status HTTP inesperado)
- [x] Test unitario: `body_mismatch`
- [x] Test unitario: check correcto → causa ausente
- [x] Test integración: migración idempotente de `error_kind`
- [x] Test integración: `insert_check` persiste y se relee `error_kind`
- [x] Test: la API de checks serializa `error_kind`

## GREEN — Implementación
- [x] Definir `FailureCause` con serde `snake_case`
- [x] Añadir `cause: Option<FailureCause>` a `CheckOutcome`
- [x] Clasificar en `HttpChecker` (pre-resolve DNS + mapeo de errores)
- [x] Clasificar en `TcpChecker` y `TlsChecker` conservando `DnsError`
- [x] Migración `ALTER TABLE checks ADD COLUMN error_kind TEXT`
- [x] Persistir `error_kind` en `insert_check` y exponerlo en `CheckResult`/API

## REFACTOR
- [x] `cargo clippy -- -D warnings` limpio
- [x] `cargo fmt --check` limpio

## Documentación
- [x] Crear `docs/failure-cause-taxonomy.md`

## Cierre
- [x] `cargo test` verde
- [x] PR a `development` (`Closes #36`)
- [x] `openspec archive failure-cause-taxonomy`
- [x] Actualizar `plans/PLAN-002.md`
