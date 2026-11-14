# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **#68 ✅ COMPLETADO** (rama `feature/dns-single-flight`, PR abierto a `development`):
  single-flight (dedupe en vuelo) en `DnsResolver`. Antes, N resoluciones concurrentes del
  mismo host en cache-miss lanzaban N consultas (thundering herd); ahora comparten una única
  consulta y su resultado vía `tokio::sync::watch`, con guard RAII `InflightGuard` (reclama la
  entrada si el líder se cancela/panickea, caso real por los `tokio::time::timeout` de los checkers).
  - Tests offline: 4 escenarios de dedupe + `cancelled_leader_is_reclaimed_and_hands_off`.
  - Auditado por `@rust-reviewer` (2 rondas; el fix del líder cancelado fue un hallazgo crítico
    corregido y re-verificado): **APROBADO**. `just check` limpio; 280+9+45+8+3 = 345 verdes.
  - Docs: `docs/dns-single-flight.md`. Plan `plans/PLAN-011-ARCHIVED.md`.
  - Change OpenSpec archivado: `openspec/changes/archive/2026-11-14-dns-single-flight/`.
- **#64 ✅ COMPLETADO** (PR #67 mergeado): `validate_token` sin recursión con JWKS vacío.
  Docs `docs/jwks-empty-no-recursion.md`; plan `plans/PLAN-010-ARCHIVED.md`.
- **#61 ✅ COMPLETADO** (PR #65 mergeado): tests de validación JWT + CI de seguridad.
  Docs `docs/jwt-validation-tests.md`, `docs/security-ci.md`; plan `plans/PLAN-009-ARCHIVED.md`.
- Producción en **v0.15.3** (imagen `ghcr.io/atareao/watchbeat:latest`, revision `147a7bcf`),
  `healthy`, 7 monitores `up`. Despliegue gestionado por Dockge.

## Pendiente

### #62 — Follow-ups Robust Check Engine (sub-issues)
- **#68** ✅ dedupe de consultas DNS en vuelo (PR abierto).
- **#69** ⚙️ canario de red configurable vía settings (**siguiente**).
- **#70** 📡 `error_kind` en el evento SSE.
- **#71** 🚦 estado del breaker (closed/open) en API y dashboard.
- **#72** 📬 agrupar notificaciones latency/expiry en el breaker.
- **#63** ♿ a11y del timeline + test SNI real.

### Pendiente inmediato
- Revisar y mergear el PR de #68. Después, seguir por **#69** y el resto de sub-issues de #62,
  cada uno en sesión separada (change OpenSpec + TDD).

### Follow-ups técnicos
- Canario de red configurable (hoy fijo a 30 s) → #69.
- Estado del breaker (`closed`/`open`) expuesto en API/dashboard → #71.
- `error_kind` en el evento SSE → #70.
- a11y del timeline (barras distinguibles solo por color) → #63.
- Agrupar `latency`/`expiry` en el breaker → #72.
- SNI real no assertado con handshake TLS completo → #63.
- `fetch_jwks` ignora el header `kid` y prueba todas las claves (ver `docs/jwt-validation-tests.md`).
- `Vec<DecodingKey>` clonado por request autenticada (preexistente).

### Housekeeping
- `libsqlite3-sys` 0.37 (RUSTSEC-2026-0459): esperar a que sqlx suba de versión.
- Doble lockfile `package-lock.json` + `pnpm-lock.yaml` (el build usa npm; pnpm redundante).
- CHANGELOG auto-generado desactualizado (entradas hasta 0.11.0; ahora con `[0.15.0]`).

## Release v0.15.3 — publicada y desplegada

Incluye v0.15.2 (Robust Check Engine, queries #33, DNS #52, ping #31) más #55 (flaky),
#57 (TCP/TLS multi-IP) y #59 (deps de seguridad). `jsonwebtoken` 12, rustls-webpki 0.103.17,
roots 1.0.12, vite ^8.3.8. `cargo audit`: solo queda `libsqlite3-sys` 0.37; `npm audit`: 0.

## Robust Check Engine (PLAN-002) — COMPLETADO

Issues #34–#40 cerrados. Docs: `docs/robust-check-engine.md`, `docs/robust-check-engine-report.md`,
`docs/adr/` (5 ADR) y un doc por fase.

## Comandos

```bash
just check                 # fmt + clippy
cd backend && cargo test   # 280 + 9 + 45 + 8 + 3
cd frontend && npx tsc --noEmit && npm run build
cd backend && cargo audit && (cd ../frontend && npm audit --audit-level=high)
```
