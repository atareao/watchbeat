# Proposal

**Issue:** [#52](https://github.com/atareao/watchbeat/issues/52)
**Plan:** `plans/PLAN-005.md`

## Why

El resolver devuelve una sola IP por host. Si el dominio tiene varios A records y
uno apunta a un servidor obsoleto, el monitor falla con `connect_refused` aunque
el target esté vivo. Caso real: `laeradelasdistros.es` (A vivo + A viejo sin 443).

## What Changes

- `DnsResolver::resolve_all` devuelve **todas** las A/AAAA, deduplicadas y con
  IPv4 primero.
- La cache guarda el `Vec<IpAddr>` completo.
- `ReqwestDnsResolver` entrega todas las direcciones a hyper, que reintenta con
  la siguiente ante un fallo de conexión.
- `resolve()` se mantiene devolviendo la primera IP (sonda de clasificación y
  `netcheck`).

## Capabilities

### Modified Capabilities

- `dns`: la resolución SHALL devolver todas las direcciones y el cliente HTTP
  SHALL hacer fallback entre ellas.

## Impact

- `backend/src/dns.rs` (resolver, cache, adaptador reqwest, upstream hickory).
- `backend/src/checker/mod.rs` (sin cambios de firma; la sonda sigue usando
  `resolve`).
- Sin cambios de API ni de esquema.
