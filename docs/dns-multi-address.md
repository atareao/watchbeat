# DNS multi-dirección con fallback (#52)

- **Issue**: [#52](https://github.com/atareao/watchbeat/issues/52)
- **Plan**: `plans/PLAN-005.md`
- **Change OpenSpec**: `openspec/changes/archive/…-dns-multi-address-fallback/`

## Qué es

El resolvedor propio de WatchBeat devuelve **todas** las direcciones A/AAAA de un
host y el cliente HTTP hace *fallback* entre ellas. Un registro DNS obsoleto ya
no provoca un falso `down`.

## Por qué

Caso real (2026-11-14): `laeradelasdistros.es` tenía dos A records:

| IP | Estado |
|---|---|
| `167.86.73.171` | servidor actual, 200 |
| `31.214.178.55` | servidor viejo, 443 refused |

El resolver devolvía **una sola** IP (`lookup.iter().next()`) y la cacheaba
30–300 s. Cuando le tocaba la obsoleta, todos los checks de esa ventana fallaban
con `connect_refused`, aunque el target estuviera vivo.

## Cómo funciona

1. `DnsResolver::resolve_all(host)` devuelve todas las direcciones,
   deduplicadas y en el orden del upstream (IPv4 primero).
2. La cache guarda el `Vec<IpAddr>` completo bajo el mismo TTL.
3. `ReqwestDnsResolver` entrega todas las direcciones a hyper, que reintenta con
   la siguiente cuando una conexión falla.
4. `resolve(host)` se mantiene devolviendo la primera IP: la usan la sonda de
   clasificación del checker HTTP y `netcheck`.

```
resolve_all("laeradelasdistros.es")
  → [167.86.73.171, 31.214.178.55]
  → hyper intenta 167.86.73.171 → 200
  (si el orden fuera el inverso: 31.214.178.55 falla → reintenta 167.86.73.171 → 200)
```

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/dns.rs` | `UpstreamAnswer.ips`, `CacheEntry.ips`, `resolve_all`, `dedupe`, adaptador reqwest, upstream hickory |
| `backend/src/checker/mod.rs` | Sin cambios de firma; la sonda sigue usando `resolve` |

## Cómo verificar

```bash
cd backend && cargo test --lib dns
```

Tests deterministas (sin red externa):

1. `resolve_all` devuelve todas las direcciones, sin duplicados.
2. La cache sirve todas las direcciones sin reconsultar el upstream.
3. `resolve` devuelve la primera (compatibilidad).
4. **Fallback real**: dos IPs cacheadas (`127.0.0.2` sin listener y `127.0.0.1`
   con un servidor) → el cliente HTTP acaba en 200.

## Alcance

- El fallback de hyper cubre **errores de conexión TCP** (p. ej. `connect_refused`
  o `network unreachable`). Si la IP obsoleta acepta TCP pero sirve un
  certificado TLS inválido, hyper no cambia de dirección: el fallback no cubre el
  handshake TLS.
- `TcpChecker` y `TlsChecker` siguen resolviendo a una sola IP (primera). Mismo
  patrón de falso `down` por A obsoleta, fuera del alcance de este cambio.

## Decisiones

- **Dedupe en el resolver, no en el upstream**: el contrato vale para cualquier
  `Upstream` (incluidos los dobles de test).
- **`resolve` se mantiene**: la sonda de clasificación solo necesita saber si el
  nombre resuelve; devolver la primera IP evita cambiar firmas en checkers y
  `netcheck`.
- **Orden IPv4 primero**: lo fija `LookupIpStrategy::Ipv4AndIpv6` de hickory.

## Follow-ups

- Si un host devuelve muchas direcciones, limitar el número que se entrega a
  hyper (p. ej. 8) para acotar el tiempo del peor caso.
- Métrica/log cuando el fallback salva un check (útil para detectar DNS sucio).
