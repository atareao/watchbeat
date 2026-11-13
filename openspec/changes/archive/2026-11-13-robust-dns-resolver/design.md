# Design

## Context

Ver `proposal.md` — Why. Hoy:
- El `HttpChecker` usa un `reqwest::Client` global que resuelve el hostname con
  el resolvedor del SO (en el contenedor: Docker embedded DNS `127.0.0.11` →
  upstreams del host). Sin caché, sin control de timeout de resolución.
- El `TcpChecker` conecta por hostname (`TcpStream::connect`), resolviendo
  también vía el SO.
- El `TlsChecker` abre un `TcpStream` al host.

No existe ninguna capa de resolución propia ni caché.

## Goals / Non-Goals

**Goals**
- Resolver los hosts con un resolvedor propio con caché TTL.
- Fallback entre varios upstreams con timeout corto por intento.
- Integración transparente en http/tcp/tls preservando SNI y `Host`.
- Error tipado que la Fase 2 pueda mapear a `FailureCause`.

**Non-Goals**
- Reintentos del check (descartados en el plan).
- Pinning permanente de IP.
- DNSSEC, DoH/DoT (a evaluar en el futuro; no necesarios para este problema).

## Decisions

- **D1 · `hickory-resolver` como motor.** Alternativas consideradas:
  `reqwest` con `resolve()` estático (no cachea), `dns_lookup` (sin caché),
  resolver del SO (el problema a eliminar). Racional: caché TTL nativa,
  control de upstreams y timeout, mantenida y madura.
- **D2 · Integración en reqwest vía `dns_resolver`.** Se implementa el trait
  `reqwest::dns::Resolve` respaldado por la caché y se pasa a
  `ClientBuilder::dns_resolver(...)`. Esto conserva automáticamente SNI y la
  cabecera `Host` (reqwest sigue creyendo que el destino es el hostname), y
  evita construir peticiones por IP a mano. Alternativa descartada: resolver a
  IP y reescribir la URL con cabecera `Host` (rompe el SNI → fallo de TLS).
- **D3 · TTL con cotas.** Se respeta el TTL del registro, acotado entre un
  mínimo y un máximo configurables (p. ej. 30 s / 300 s) para no martillear los
  upstreams ni servir IPs obsoletas.
- **D4 · Fallback multi-upstream.** Upstreams por defecto (`1.1.1.1`,
  `8.8.8.8`, `9.9.9.9`); si el primero no responde dentro del timeout, se pasa
  al siguiente. Override opcional por env.
- **D5 · Error tipado `DnsError`.** Variantes `Timeout`, `Servfail`,
  `Nxdomain`, `Other`. Es el contrato que consume la Fase 2 (taxonomía de causas).
- **D6 · TCP/TLS.** El `TcpChecker` resuelve con `dns::resolve` y conecta al
  `SocketAddr`; el `TlsChecker` hace lo propio antes de negociar TLS (SNI con el
  hostname original).

## Risks / Trade-offs

- [Nueva dependencia] → confinada en `backend/src/dns.rs`; API interna estrecha.
- [Caché sirve IP obsoleta tras cambio de IP del target] → TTL máximo acotado.
- [El resolvedor también usa la red] → los upstreams por IP evitan depender del
  DNS del host; si todo falla, se devuelve error tipado (Fase 2/3 decidirán).
- [TCP/TLS requieren tocar dos checkers] → cubierto por tests de integración.

## Migration Plan

- Sin migración de datos. Cambio incremental tras un flag interno si fuese
  necesario revertir. Despliegue normal (contenedor). Rollback = revertir el PR.

## Open Questions

- ¿Exponer los upstreams por env (`WATCHBEAT_DNS_UPSTREAMS`) ya en esta fase o
  fijar defaults internos y dejarlo configurable más adelante? (No cambia las
  specs; se resuelve durante la implementación.)
