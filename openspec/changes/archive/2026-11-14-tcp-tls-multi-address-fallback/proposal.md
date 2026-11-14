# Proposal

**Issue:** [#57](https://github.com/atareao/watchbeat/issues/57)
**Plan:** `plans/PLAN-007.md`

## Why

`TcpChecker` y `TlsChecker` resuelven con `dns::resolve` (una sola IP). Con
varios A/AAAA y uno obsoleto, el monitor da `down` aunque otra dirección esté
viva. Es el mismo falso positivo que #52 corrigió para HTTP.

## What Changes

- TCP y TLS resuelven con `dns::resolve_all` y prueban cada dirección.
- Éxito si cualquiera conecta (TLS: handshake completo).
- Si todas fallan, causa tipada determinista (primera) + resumen de intentos.
- Variantes inyectables multi-dirección para tests; las actuales se mantienen.

## Capabilities

### Modified Capabilities

- `checker`: TCP/TLS SHALL hacer fallback entre todas las direcciones resueltas.

## Impact

- `backend/src/checker/mod.rs` (TcpChecker).
- `backend/src/checker/tls.rs` (TlsChecker).
- Sin cambios de API ni de esquema.
