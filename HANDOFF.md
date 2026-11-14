# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **Release v0.15.3 publicada y desplegada** (tag `v0.15.3`, `main` + sync
  `development`). Incluye v0.15.2 (Robust Check Engine, queries #33, DNS #52,
  ping #31) más #55 (flaky), #57 (TCP/TLS multi-IP) y #59 (deps de seguridad).
- `main` y `development` sincronizados. Tests: **266** lib + 9 + 45 + 8 + 3 verdes.
- **#59** ✅ (PR #60): deps de seguridad actualizadas. `jsonwebtoken` 12
  (rsa 0.10, crypto-bigint 0.7, ctutils 0.4.8), rustls-webpki 0.103.17,
  roots 1.0.12, vite ^8.3.8. `cargo audit`: solo queda `libsqlite3-sys` 0.37
  (bloqueado por sqlx 0.9.0). `npm audit`: 0. **18 alertas Dependabot cerradas**
  (stale). Doc `docs/security-deps.md`, PLAN-008 archivado.
- **#55** ✅ (PR #56): flaky de conexión rechazada eliminado (puerto 1, fuera del
  rango efímero). Doc `docs/testing-connect-refused.md`, PLAN-006 archivado.
- **#57** ✅ (PR #58): fallback multi-dirección en TCP/TLS (`resolve_all` + bucle
  por dirección, timeout compartido, causa + resumen). Doc
  `docs/tcp-tls-multi-address.md`, PLAN-007 archivado. Auditado 2 veces (APROBADO).
- Ramas de feature/hotfix/release limpiadas (locales y remotas).

### Producción (co1) — DESPLEGADO v0.15.3
- Imagen `ghcr.io/atareao/watchbeat:latest`, revision `147a7bcf` (v0.15.3).
- `healthy`, `/health` OK; 7 monitores `up` tras el despliegue; `ping` OK.
- `ping` funciona sin `cap_add` porque Docker permite ICMP por
  `net.ipv4.ping_group_range`; el compose del repo mantiene `NET_RAW` como
  defensa. El compose de Dockge no lo necesita.
- Contenedor recreado con `docker run` preservando env, red `proxy` y volumen
  `watchbeat_watchbeat_data`; añadidos `--cap-add NET_RAW` y healthcheck.
- Verificado: `healthy`, `CAP_NET_RAW` activo, `ping` como usuario `app` OK,
  `/health` → `{"status":"ok"}`. Monitores sin `down`/`error` desde el fix
  (todo `up`; `laeradelasdistros` estable).
- **Despliegue gestionado por Dockge**: no hay compose visible en
  `/opt/stacks/watchbeat` porque Dockge administra el stack. Recreaciones
  futuras deben hacerse desde Dockge (o `docker run` puntual preservando env,
  red `proxy`, volumen `watchbeat_watchbeat_data` y `--cap-add NET_RAW`).

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
- **#61** 🧪 Tests de validación JWT + CI de seguridad (**siguiente**).
- **#62** ✨ Follow-ups Robust Check Engine (canario, breaker, SSE, agrupación).
- **#63** ♿ a11y del timeline + test SNI real.

### Completado recientemente
- **#33** ✅ (PR #51): índice cubriente `idx_checks_timeline`, `ANALYZE` y tests de
  plan de ejecución. Plan archivado en `plans/PLAN-003-ARCHIVED.md`; doc en
  `docs/checks-query-perf.md`.
- **#52** ✅ (PR #53): DNS multi-dirección con fallback en hyper. Plan archivado
  en `plans/PLAN-005-ARCHIVED.md`; doc en `docs/dns-multi-address.md`.
- **#31** ✅ (PR #54 + validación en producción): monitor `ping-validacion-31`
  → `1.1.1.1` reporta `up` con `response_time_ms=9` (parseado de `time=`).
  Diagnóstico y permisos de la sonda ping. `PingChecker`
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
- SNI real no assertado con handshake TLS completo.

### Housekeeping
- `libsqlite3-sys` 0.37 (RUSTSEC-2026-0459): esperar a que sqlx suba de versión.
- Añadir tests de validación JWT (gap preexistente).
- Doble lockfile `package-lock.json` + `pnpm-lock.yaml` (el build usa npm; pnpm redundante).
- CHANGELOG auto-generado desactualizado (entradas hasta 0.11.0; ahora con `[0.15.0]`).

### Pendiente inmediato
- Empezar por **#61** en sesión nueva (change OpenSpec + TDD). El resto, en
  sesiones separadas por issue.

### Housekeeping
- Ramas remotas mergeadas ya inexistentes; refs locales podadas.
- `frontend/pnpm-lock.yaml` eliminado (el build usa npm).

## Comandos

```bash
just check                 # fmt + clippy
cd backend && cargo test   # 266 + 9 + 45 + 8 + 3
cd frontend && npx tsc --noEmit && npm run build
```
