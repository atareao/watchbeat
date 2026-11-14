# Proposal

**Issue:** [#35](https://github.com/atareao/watchbeat/issues/35)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 1)

## Why

Los checkers resuelven el hostname con el DNS del sistema operativo en cada
intento y sin caché. Cuando el DNS del host sufre un microcorte (~5 s), **todos**
los monitores fallan a la vez y se registran como `down`/`error` aunque los
servicios estén perfectamente operativos. Es la causa de los falsos positivos
masivos detectados en producción (4 incidentes correlacionados en 2 semanas).

## What Changes

- **Nueva capacidad `dns`**: resolvedor embebido con caché respetando TTL,
  múltiples upstreams y fallback, independiente del DNS del sistema operativo.
- `backend/src/dns.rs`: API interna `resolve(host) -> Result<IpAddr, DnsError>`
  con error tipado (`Timeout`, `Servfail`, `Nxdomain`).
- Integración transparente en los checkers (`http`, `tcp`, `tls`): la IP se
  obtiene del resolvedor propio preservando SNI y cabecera `Host`.
- La resolución deja de estar en el camino crítico: ante un corte transitorio
  del DNS del host, la IP en caché mantiene el check funcionando.

No hay cambios *breaking*: la API pública HTTP no cambia y el comportamiento
observable (checks correctos) se preserva; solo se elimina una dependencia frágil.

## Capabilities

### New Capabilities
- `dns`: resolución de nombres propia, con caché TTL y fallback multi-upstream.

### Modified Capabilities
_(ninguna)_

## Impact

- **Código**: `backend/src/dns.rs` (nuevo), `backend/src/checker/mod.rs`,
  `backend/src/checker/tls.rs`, `backend/src/lib.rs`.
- **Dependencia nueva**: `hickory-resolver`.
- **Configuración**: upstreams por defecto; override opcional por env/settings
  (se detalla en `design.md`).
- **Sin migración de DB**, sin cambios de API ni de frontend.
