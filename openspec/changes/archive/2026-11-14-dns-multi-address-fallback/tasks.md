To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: `resolve_all` devuelve todas las IPs del upstream
- [x] RED: `resolve_all` cachea todas y no reconsulta dentro del TTL
- [x] RED: `resolve` devuelve la primera IP (compatibilidad)
- [x] RED: fallback real — dos IPs cacheadas (una muerta, una viva) → check OK
- [x] GREEN: `UpstreamAnswer`/`CacheEntry`/`resolve_all`/`ReqwestDnsResolver`
- [x] GREEN: `HickoryUpstream` recoge todas las A/AAAA
- [x] REFACTOR: clippy/fmt limpios, tests verdes

## Cierre

- [x] `docs/dns-multi-address.md`
- [x] `openspec archive dns-multi-address-fallback`
- [x] PR a `development` con `Closes #52`
