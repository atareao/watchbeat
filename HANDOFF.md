# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **Release v0.15.2 publicada** (tag `v0.15.2`, merge a `main` + sync `development`).
  Incluye el **Robust Check Engine** (v0.15.0), la optimización de queries (#33,
  v0.15.1), el DNS multi-dirección (#52) y el diagnóstico/permisos de ping (#31).
- `main` y `development` sincronizados. Tests: **254** lib + 9 + 45 + 8 + 3 verdes.
- Ramas de feature/hotfix/release limpiadas (locales y remotas).

### Producción (co1) — DESPLEGADO v0.15.2
- Imagen `ghcr.io/atareao/watchbeat:latest` (workflow Docker OK).
- Contenedor recreado con `docker run` preservando env, red `proxy` y volumen
  `watchbeat_watchbeat_data`; añadidos `--cap-add NET_RAW` y healthcheck.
- Verificado: `healthy`, `CAP_NET_RAW` activo, `ping` como usuario `app` OK,
  `/health` → `{"status":"ok"}`. Monitores sin `down`/`error` desde el fix
  (todo `up`; `laeradelasdistros` estable).
- **Caveat**: el compose de producción `/opt/stacks/watchbeat/compose.yaml`
  (referenciado por labels) ya no existe en el host. Recreaciones futuras
  deben hacerse con `docker run` o restaurando ese compose.

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
- (ninguno)

### Completado recientemente
- **#33** ✅ (PR #51): índice cubriente `idx_checks_timeline`, `ANALYZE` y tests de
  plan de ejecución. Plan archivado en `plans/PLAN-003-ARCHIVED.md`; doc en
  `docs/checks-query-perf.md`.
- **#52** ✅ (PR #53): DNS multi-dirección con fallback en hyper. Plan archivado
  en `plans/PLAN-005-ARCHIVED.md`; doc en `docs/dns-multi-address.md`.
- **#31** ✅ (PR #54): diagnóstico y permisos de la sonda ping. `PingChecker`
  captura stderr/exit code, clasifica permisos y binario ausente como `error`,
  DNS como `down`, y pasa el target tras `--`. Imagen con `iputils`/`libcap`/
  `setcap` + `chmod u-s`, compose con `cap_add: [NET_RAW]`. Plan archivado en
  `plans/PLAN-004-ARCHIVED.md`; doc en `docs/ping-checker.md`.

### Follow-ups técnicos
- Canario de red configurable (hoy fijo a 30 s).
- Estado del breaker (`closed`/`open`) expuesto en API/dashboard.
- Dedupe de consultas DNS en vuelo.
- `error_kind` en el evento SSE (hoy se rellena con refetch).
- a11y del timeline (barras distinguibles solo por color).
- Agrupar `latency`/`expiry` en el breaker.
- Test flaky preexistente `tcp_check_should_report_connect_refused_cause`
  (`closed_loopback_port` reutiliza puerto efímero bajo paralelismo).
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
