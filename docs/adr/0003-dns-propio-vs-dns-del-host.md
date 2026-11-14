# ADR-0003 · DNS propio con caché en lugar del DNS del host

- **Estado**: aceptado
- **Fecha**: 2026-11-14
- **Contexto**: PLAN-002, Fase 1

## Contexto

Los checks HTTP/TCP/TLS resolvían nombres con el DNS del sistema operativo (Docker
embedded DNS / systemd-resolved). Un microcorte de ese DNS hacía fallar **todos** los
checks a la vez.

## Decisión

Implementar un **resolvedor DNS propio** (`backend/src/dns.rs`, motor `hickory-resolver`)
con **caché respetando TTL**, **múltiples upstreams con fallback** y **error tipado**
(`DnsError`), e inyectarlo en reqwest vía `reqwest::dns::Resolve`.

## Razones

- La **caché TTL** hace que un corte transitorio del DNS del host no rompa los checks: la
  IP ya resuelta sigue disponible hasta que expire su TTL.
- Los upstreams se configuran **por IP literal** (`1.1.1.1`, `8.8.8.8`, `9.9.9.9`), así no
  se depende del DNS del host para resolverlos.
- El **error tipado** (`Timeout`/`Servfail`/`Nxdomain`/`Other`) alimenta la taxonomía de la
  Fase 2 sin adivinar a partir de cadenas de error de reqwest.
- Inyectar el resolver en reqwest (en vez de reescribir la URL con cabecera `Host`)
  **preserva SNI** y la cabecera `Host`, evitando fallos de TLS.

## Consecuencias

- Nueva variable `WATCHBEAT_DNS_UPSTREAMS` (con defaults sensatos).
- `SERVFAIL` → fallback al siguiente upstream; `NXDOMAIN` → corte (autoritativo).
- Sin deduplicación de consultas en vuelo (trade-off conocido).
