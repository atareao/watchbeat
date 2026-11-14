# Design

## Context

Ver `proposal.md` — Why. Estado actual relevante:

- **Timeline**: inline en `frontend/src/pages/MonitorDetail.tsx` (`HealthLatencyChart` 571–740,
  `HeartbeatChart` 447–569). Estados por **color inline** (`healthColor()`, `STATUS_TAG`); único
  texto por barra = `Tooltip` en hover; sin `role`/`tabIndex`/`aria-label`. **Sin test runner de
  frontend** (no hay vitest).
- **Colores de estado duplicados** en `MonitorDetail.tsx`, `MonitorCard.tsx` (`STATUS_CONFIG`) y
  `Dashboard.tsx`.
- **TLS**: `checker/tls.rs` usa `ServerName::try_from(host)` → `connector.connect(server_name,
  tcp)`; roots solo `webpki-roots`; **sin `rcgen`**; ningún test asserta el SNI.

## Goals / Non-Goals

**Goals**: timeline distinguible sin color, con texto accesible y navegable por teclado; tokens de
estado centralizados; test de SNI determinista sin red externa.

**Non-Goals**: rediseñar el timeline; introducir un test runner de frontend (los criterios de #63
piden `tsc --noEmit` + build + tests Rust verdes); cambiar API/DB/esquema; sustituir `webpki-roots`
por `native-certs`.

## Decisions

### D1 — Canal no cromático: patrón CSS + icono
Cada estado define además del color un `backgroundImage` con `repeating-linear-gradient` (rayado
con ángulo/periodo distinto por estado) y un icono en el detalle. Alternativa considerada: overlay
SVG por barra (más nodos y coste de render); se elige CSS por coste ~0 y por funcionar en escala
de grises.

### D2 — Token único de estado
Nuevo módulo `frontend/src/theme/statusStyles.ts` que exporta `STATUS_STYLES: Record<Status, {
color, pattern, label, icon }>`. `MonitorDetail`, `MonitorCard` y `Dashboard` lo consumen.
Alternativa: mantener los mapas actuales (rechazada: ya divergen entre ficheros).

### D3 — Accesibilidad por barra
Cada barra: `role="img"`, `tabIndex={0}`, `aria-label` compuesto (estado + %UP + latencia +
nº checks + marca temporal) y `Tooltip trigger={["hover","focus"]}` con foco visible
(`:focus-visible`). Alternativa: `aria-hidden` + tabla resumen (más complejo, peor UX).

### D4 — Añadir `warning` al mapa de UI
`warning` (ámbar + rayado diagonal) se incorpora al token; hoy el backend lo emite en TLS/cert.

### D5 — Test SNI: `rcgen` + servidor `tokio-rustls` que captura el SNI
Añadir `rcgen` como **dev-dependency**; generar un certificado para `example.test`; levantar un
servidor `tokio-rustls` con un `ResolvesServerCert` que lea `client_hello.server_name()` y lo
envíe por canal al test. El test ejecuta `TlsChecker` contra `127.0.0.1:<puerto>` inyectando el
resolver DNS para que `example.test` resuelva a loopback (patrón ya usado en los tests actuales).
Alternativas: `openssl s_server` (no determinista ni portable); proxy TCP (no ve el SNI sin
descifrar). Se elige `rcgen`+rustls por ser 100% en proceso.

## Risks / Trade-offs

- [El `Tooltip` de antd podría no mostrarse en foco en algunos navegadores] → `trigger=["hover","focus"]`
  es soportado; validación con build + revisión manual (no hay runner de frontend).
- [`rcgen` añade una dependencia] → es dev-dependency: no entra en el binario de producción.
- [Patrón invisible en barras muy estrechas] → el `aria-label` y el detalle del tooltip cubren la
  distinción aunque el patrón no se aprecie.

## Migration Plan

Sin migraciones de datos. Rollback = revertir el PR. No requiere feature flag.

## Open Questions

Ninguna que cambie las specs, el enfoque o el desglose de tareas.
