# PLAN-007 · Fallback multi-dirección en TCP/TLS (#57)

- **Issue**: [#57](https://github.com/atareao/watchbeat/issues/57)
- **Rama**: `feature/tcp-tls-multi-address-fallback`
- **Change OpenSpec**: `openspec/changes/tcp-tls-multi-address-fallback/`

## Causa

TCP/TLS usan `dns::resolve` (primera IP). Un A obsoleto provoca falsos `down`.

## Solución

`resolve_all` + bucle sobre direcciones. Éxito si alguna conecta (TLS: handshake).
Si todas fallan, causa de la primera + resumen de intentos.

## Tareas (TDD)

- [x] RED: TCP 2 IPs (muerta/viva) → up.
- [x] RED: TLS 2 IPs (muerta/viva) → up.
- [x] RED: todas muertas → down/connect_refused.
- [x] GREEN: TcpChecker y TlsChecker multi-dirección.
- [x] REFACTOR: clippy/fmt + suite.
- [x] Docs, archive, PR `Closes #57`.
