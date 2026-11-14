# Robust DNS Resolver (Fase 1 · Robust Check Engine)

- **Issue**: [#35](https://github.com/atareao/watchbeat/issues/35)
- **Plan**: `plans/PLAN-002-ARCHIVED.md` — Robust Check Engine (Fase 1 de 6)
- **Change OpenSpec**: `openspec/changes/archive/…-robust-dns-resolver/`

## Qué es

Resolvedor DNS propio, embebido en el backend, con **caché respetando TTL**,
**múltiples upstreams con fallback** y **error tipado**. Sustituye la resolución
implícita del DNS del sistema operativo (Docker embedded DNS / systemd-resolved)
en los checks HTTP, TCP y TLS.

## Por qué

En producción, un microcorte (~5 s) del DNS del host hacía que **todos** los
monitores fallaran a la vez y se registraran como `down`/`error`, aunque los
servicios funcionaran. El fallo ocurría en la fase de resolución de nombres,
antes de abrir ninguna conexión. Ver `HANDOFF.md` e issue #34.

Con la caché propia, un corte transitorio del DNS del host ya no rompe los
checks: la IP ya resuelta sigue en caché hasta que expire su TTL.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/dns.rs` | **Nuevo.** Resolvedor, caché, fallback, `DnsError`, `reqwest::dns::Resolve` |
| `backend/src/checker/mod.rs` | `http_client()` usa el `dns_resolver` propio; `TcpChecker` resuelve con `dns::resolve` |
| `backend/src/checker/tls.rs` | `TlsChecker` resuelve con `dns::resolve` antes del handshake |
| `backend/src/config.rs` | `Config.dns_upstreams` + `dns_upstreams_from_env()` |
| `backend/src/main.rs` | `dns::init(config.dns_upstreams)` en el arranque |
| `backend/src/lib.rs` | Export del módulo `dns` |

## Cómo funciona

1. **Resolución**: `dns::resolve(host) -> Result<IpAddr, DnsError>`. Si el host ya
   es una IP literal, se devuelve tal cual (sin consultar upstreams).
2. **Caché**: clave canónica (minúsculas, sin punto final, sin corchetes). TTL del
   registro acotado entre `min_ttl` (30 s) y `max_ttl` (300 s). Cota dura de
   entradas (`MAX_CACHE_ENTRIES`), con evicción de las expiradas o de la de menor
   `expires_at`.
3. **Fallback**: se prueban los upstreams en orden con un timeout corto por
   intento. Si el primero no responde, se pasa al siguiente. `SERVFAIL` continúa
   al siguiente upstream; `NXDOMAIN` corta (es autoritativo). Si **todos** fallan
   se devuelve `DnsError::Timeout` (nunca un falso positivo).
4. **Error tipado**: `DnsError::{Timeout, Servfail, Nxdomain, Other}` — contrato
   que la Fase 2 mapeará a `FailureCause`.
5. **Integración reqwest**: `ReqwestDnsResolver` implementa `reqwest::dns::Resolve`
   y se inyecta vía `ClientBuilder::dns_resolver(...)`, devolviendo
   `SocketAddr(ip, 0)` (reqwest rellena el puerto). Así reqwest sigue creyendo que
   el destino es el hostname y **conserva SNI y cabecera `Host`**.
6. **TCP/TLS**: resuelven con `dns::resolve` y conectan al `SocketAddr`. En ambos,
   resolución + conexión van **dentro del mismo `tokio::time::timeout`** del
   monitor. Un fallo de DNS se reporta como `status:"error"` (no `"down"`): es un
   fallo de infraestructura, no del servicio.

## Variables / configuración

| Variable | Default | Descripción |
|---|---|---|
| `WATCHBEAT_DNS_UPSTREAMS` | `1.1.1.1,8.8.8.8,9.9.9.9` | Lista de upstreams separados por coma. Parseo tolerante: entradas inválidas se ignoran; lista vacía → defaults. |

Constantes internas: TTL mínimo 30 s, TTL máximo 300 s.

## Cómo habilitar / desplegar

- **Activo por defecto**, sin migración de DB ni cambios de API/frontend.
- Sin `WATCHBEAT_DNS_UPSTREAMS` funciona con los upstreams por defecto (todos por
  IP, así no se depende del DNS del host para resolverlos).
- Rollback: revertir el PR.

## Decisiones

- **`hickory-resolver`** como motor (caché TTL nativa, control de upstreams).
- **`dns_resolver` de reqwest** en lugar de reescribir la URL con cabecera `Host`
  (esto último rompería el SNI → fallo de TLS).
- **`cache_size = 0` en hickory**: la caché propia es la única fuente de verdad
  del TTL.
- **SERVFAIL → fallback; NXDOMAIN → corte** (ver arriba).

## Tests

16 unitarios en `dns.rs` + 5 en `checker/mod.rs` + 3 en `checker/tls.rs`, todos
deterministas y **sin red externa** (upstream falso inyectado + `prime_cache`).
Cubren: host nuevo, caché dentro/fuera de TTL, cotas de TTL, cota de entradas,
fallback, todos caídos → `Timeout`, NXDOMAIN, SERVFAIL, IP literal, clave
canónica, `split_host_port`, check HTTP con IP de caché (`Host` preservado),
check TCP/TLS con el resolvedor propio y fallo DNS → `error`.

## Follow-ups

- **Fase 2** (`#36`): mapear `DnsError` a `FailureCause`.
- **Issue #41**: bug preexistente — `rustls` paniquea por `CryptoProvider` ambiguo
  (`ring` + `aws-lc-rs`); los monitores TLS revientan en producción.
- El **SNI real** no se asserta con handshake TLS completo (se preserva por
  diseño al no reescribir la URL).
- Sin deduplicación de consultas en vuelo (thundering herd): N resolves
  concurrentes del mismo host miss consultan N veces. Trade-off conocido.
