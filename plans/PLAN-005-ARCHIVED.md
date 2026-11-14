# PLAN-005 · DNS multi-dirección con fallback (#52)

- **Issue**: [#52](https://github.com/atareao/watchbeat/issues/52)
- **Rama**: `feature/dns-multi-address-fallback`
- **Change OpenSpec**: `openspec/changes/dns-multi-address-fallback/`

## Síntoma

Un monitor HTTP reporta `down` con `connect_refused` aunque el target esté vivo,
si el dominio tiene varios A records y uno apunta a un servidor obsoleto.

Caso real: `laeradelasdistros.es` tenía `167.86.73.171` (vivo) y
`31.214.178.55` (viejo, sin 443). El resolver devolvía una sola IP y la cacheaba;
cuando le tocaba la obsoleta, fallaban todos los checks de esa ventana.

## Causa

`backend/src/dns.rs` resuelve a **una sola** dirección:

- `UpstreamAnswer { ip, ttl }`
- `HickoryUpstream::query` → `lookup.iter().next()`
- `ReqwestDnsResolver` → un único `SocketAddr`
- `CacheEntry { ip, expires_at }`

## Solución

1. `UpstreamAnswer { ips: Vec<IpAddr>, ttl }` y `CacheEntry { ips, expires_at }`.
2. `DnsResolver::resolve_all(host) -> Result<Vec<IpAddr>, DnsError>`:
   - IP literal → `vec![ip]`.
   - Cache hit → clon del `Vec`.
   - Upstream → todas las A/AAAA, deduplicadas, IPv4 primero.
3. `resolve(host)` se mantiene: devuelve la primera IP (sonda de clasificación,
   `netcheck`).
4. `ReqwestDnsResolver` devuelve **todas** las direcciones; hyper reintenta con
   la siguiente ante `connect_refused`.
5. `prime_cache` sigue aceptando una IP; se añade `prime_cache_all` para tests.

## Tareas (TDD)

- [x] RED: `resolve_all` devuelve todas las IPs del upstream.
- [x] RED: `resolve_all` cachea todas y no reconsulta dentro del TTL.
- [x] RED: `resolve` devuelve la primera IP (compatibilidad).
- [x] RED: fallback real — dos IPs cacheadas (una muerta, una viva) → check OK.
- [x] GREEN: `UpstreamAnswer`/`CacheEntry`/`resolve_all`/`ReqwestDnsResolver`.
- [x] GREEN: `HickoryUpstream` recoge todas las A/AAAA.
- [x] REFACTOR: clippy/fmt, tests verdes.
- [x] Docs: `docs/dns-multi-address.md`.
- [x] PR a `development` con `Closes #52`.
