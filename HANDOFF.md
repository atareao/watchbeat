# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **Release v0.15.1 publicada** (tag `v0.15.1`, merge a `main` + sync `development`).
  Incluye el **Robust Check Engine** (v0.15.0) y la optimización de queries de
  checks (#33, v0.15.1).
- `main` y `development` sincronizados. Tests: 238 + 9 + 45 + 3 verdes.
- Ramas de feature/hotfix/release limpiadas (locales y remotas).

## Robust Check Engine (PLAN-002) — COMPLETADO

Plan archivado en `plans/PLAN-002-ARCHIVED.md`. Issues #34–#40 cerrados.

| Fase | Issue | PR |
|---|---|---|
| 1 · DNS propio con caché | #35 | #42 |
| 2 · Taxonomía `FailureCause` + `error_kind` | #36 | #43 |
| 3 · Canario + atribución `indeterminate` | #37 | #44 |
| 4 · Jitter de ticks | #38 | #45 |
| 5 · Breaker anti-tormenta | #39 | #46 |
| 6 · Documentación y cierre | #40 | #47 |

Docs: `docs/robust-check-engine.md` (global), `docs/robust-check-engine-report.md`
(informe), `docs/adr/` (5 ADR), y un doc por fase.

## Hotfix #41 — RESUELTO

PR #48: `rustls::crypto::ring::default_provider().install_default()` al arrancar
(`backend/src/main.rs`). Evita el panic de los monitores TLS por `CryptoProvider` ambiguo.

## Dependabot

PR #49: `vite` → 8.3.8, `source-map-js` → 1.2.2, `rustls` → 0.23.46.
`npm audit` y `pnpm audit` → 0 vulnerabilidades. PRs #29/#30/#32 cerrados.

> Las 18 alertas de Dependabot siguen abiertas en la API porque el escaneo de la rama
> por defecto aún no se ha refrescado; los manifiestos de `main` ya tienen las versiones
> corregidas. Se cerrarán en el siguiente re-escaneo.

## Pendiente

### Issues abiertos
- **#31** — Error en la sonda tipo ping (bug funcional, sin diagnosticar).

### Completado recientemente
- **#33** ✅ (PR #51): índice cubriente `idx_checks_timeline`, `ANALYZE` y tests de
  plan de ejecución. Plan archivado en `plans/PLAN-003-ARCHIVED.md`; doc en
  `docs/checks-query-perf.md`.
- **#52** ✅ (PR #53): DNS multi-dirección con fallback en hyper. Plan archivado
  en `plans/PLAN-005-ARCHIVED.md`; doc en `docs/dns-multi-address.md`.

### Follow-ups técnicos
- Canario de red configurable (hoy fijo a 30 s).
- Estado del breaker (`closed`/`open`) expuesto en API/dashboard.
- Dedupe de consultas DNS en vuelo.
- `error_kind` en el evento SSE (hoy se rellena con refetch).
- a11y del timeline (barras distinguibles solo por color).
- Agrupar `latency`/`expiry` en el breaker.
- SNI real no assertado con handshake TLS completo.

### Housekeeping
- Doble lockfile `package-lock.json` + `pnpm-lock.yaml` (el build usa npm; pnpm redundante).
- CHANGELOG auto-generado desactualizado (entradas hasta 0.11.0; ahora con `[0.15.0]`).

## Comandos

```bash
just check                 # fmt + clippy
cd backend && cargo test   # 238 + 9 + 45 + 3
cd frontend && npx tsc --noEmit && npm run build
```
