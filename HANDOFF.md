# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **#69 ✅ COMPLETADO** (rama `feature/canary-interval-setting`, PR **#74** abierto a `development`):
  canario de red configurable vía setting `canary_interval_secs` (default 30 s, rango [5, 3600],
  clamp), con recarga en caliente (patrón del breaker). `main.rs` resuelve el intervalo desde la
  BD al arrancar; `routes/settings.rs` y `routes/export_import.rs` lo re-aplican; UI Ajustes → Red.
  Tests: **355 verdes**; test de adopción en caliente determinista (tokio `start_paused`).
  Auditado por `@rust-reviewer` y `@react-reviewer` (**APROBADOS**).
  Docs `docs/canary-interval-setting.md`; plan `plans/PLAN-012-ARCHIVED.md`.
  Change OpenSpec archivado: `openspec/changes/archive/2026-11-14-canary-interval-setting/`.
- **#68 ✅ COMPLETADO** (PR #73 mergeado): single-flight (dedupe en vuelo) en `DnsResolver`.
  Docs `docs/dns-single-flight.md`; plan `plans/PLAN-011-ARCHIVED.md`.
- **#64 ✅ COMPLETADO** (PR #67 mergeado): `validate_token` sin recursión con JWKS vacío.
- **#61 ✅ COMPLETADO** (PR #65 mergeado): tests de validación JWT + CI de seguridad.
- Producción en **v0.15.3** (imagen `ghcr.io/atareao/watchbeat:latest`, revision `147a7bcf`),
  `healthy`, 7 monitores `up`. Despliegue gestionado por Dockge.

## Pendiente

### #62 — Follow-ups Robust Check Engine (sub-issues)
- **#69** ✅ canario de red configurable (PR #74 abierto).
- **#70** 📡 `error_kind` en el evento SSE (**siguiente**).
- **#71** 🚦 estado del breaker (closed/open) en API y dashboard.
- **#72** 📬 agrupar notificaciones latency/expiry en el breaker.
- **#63** ♿ a11y del timeline + test SNI real.

### Pendiente inmediato
- Revisar y mergear el PR **#74** de #69. Después, seguir por **#70** y el resto de sub-issues
  de #62, cada uno en sesión separada (change OpenSpec + TDD).

### Housekeeping
- `just check-spec` NO existe como receta del `.justfile` pese a referenciarla AGENTS.md; la
  puerta SDD queda cubierta por `opens,pec validate <change>`.
- `frontend/dist/` está trackeado pese a `dist/` en `.gitignore` (el SPA se embebe vía `include_dir!`).
- `libsqlite3-sys` 0.37 (RUSTSEC-2026-0459): esperar a que sqlx suba de versión.
- Doble lockfile `package-lock.json` + `pnpm-lock.yaml` (el build usa npm; pnpm redundante).
- CHANGELOG auto-generado desactualizado (entradas hasta 0.11.0; ahora con `[0.15.0]`).

## Release v0.15.3 — publicada y desplegada

Incluye v0.15.2 (Robust Check Engine, queries #33, DNS #52, ping #31) más #55 (flaky),
#57 (TCP/TLS multi-IP) y #59 (deps de seguridad). `jsonwebtoken` 12, rustls-webpki 0.103.17,
roots 1.0.12, vite ^8.3.8. `cargo audit`: solo queda `libsqlite3-sys` 0.37; `npm audit`: 0.

## Comandos

```bash
just check                 # fmt + clippy
cd backend && cargo test   # 289 + 9 + 46 + 8 + 3
cd frontend && npx tsc --noEmit && npm run build
cd backend && cargo audit && (cd ../frontend && npm audit --audit-level=high)
```