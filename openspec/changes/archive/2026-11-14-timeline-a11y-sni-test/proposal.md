# Proposal

## Why

El timeline de checks de `MonitorDetail` distingue los estados **solo por color** (hex inline en
`healthColor()` / `STATUS_TAG`), sin texto accesible ni navegación por teclado: es ilegible para
usuarios con daltonismo y para lectores de pantalla. Además, el checker TLS construye el
`ServerName` del SNI a partir del hostname, pero **ningún test lo verifica**: una regresión que
dejara de enviar el SNI correcto (o lo tomara de la IP resuelta) pasaría desapercibida hasta
romper handshakes contra servidores con vhosts.

## What Changes

- **Timeline accesible**: cada barra SHALL ser distinguible sin depender del color (patrón/textura
  + icono) y SHALL exponer un texto accesible (`aria-label`) con su estado y datos; el timeline
  SHALL ser navegable con teclado (el foco muestra el detalle, no solo el hover).
- **Tokens de estado centralizados**: color + patrón + etiqueta + icono por estado en un único
  módulo, reutilizado por timeline, tarjetas y dashboard (hoy duplicado en 3 ficheros).
- **Estado `warning` en UI**: se añade al mapa de estados del timeline (hoy el backend lo emite en
  TLS/cert pero la UI no lo distingue).
- **Test SNI real y determinista**: servidor TLS local con certificado de test que asserta que el
  ClientHello del checker lleva el hostname correcto como SNI; sin red externa.
- **Sin breaking changes**: no hay cambios de esquema, de API ni de comportamiento observable del
  backend más allá de blindar por test el SNI ya enviado.

## Capabilities

### New Capabilities

- `frontend-timeline-a11y`: accesibilidad del timeline de checks (distinguible sin color, texto
  accesible por barra, navegación por teclado).

### Modified Capabilities

- `checker`: se añade el requisito explícito de que el checker TLS envía el hostname del monitor
  como SNI (hoy implícito y no verificado).
- `testing`: se añade un test determinista de SNI con servidor TLS local.

## Impact

- **Frontend**: `frontend/src/pages/MonitorDetail.tsx` (`HealthLatencyChart`, `HeartbeatChart`),
  nuevo módulo de tokens de estado, y posible reutilización en `MonitorCard.tsx` / `Dashboard.tsx`.
- **Backend**: `backend/src/checker/tls.rs` (tests), `backend/Cargo.toml` (dev-dependency `rcgen`).
- **Sin migraciones** de base de datos. Sin cambios de API ni de esquema.
