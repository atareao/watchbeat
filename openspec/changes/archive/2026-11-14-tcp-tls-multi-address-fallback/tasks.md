To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: TCP con 2 IPs (una muerta, una viva) → up
- [x] RED: TLS con 2 IPs (una muerta, una viva) → up
- [x] RED: todas muertas → down/connect_refused con resumen
- [x] GREEN: `TcpChecker` multi-dirección
- [x] GREEN: `TlsChecker` multi-dirección
- [x] REFACTOR: clippy/fmt, suite completa

## Cierre

- [x] `docs/tcp-tls-multi-address.md`
- [x] `openspec archive tcp-tls-multi-address-fallback`
- [x] PR a `development` con `Closes #57`
