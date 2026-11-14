# HANDOFF — WatchBeat

## Estado (14 Nov 2026)

- **#61 ✅ COMPLETADO** (PR **#65** mergeado en `development`, commit `d1ea8fa0`):
  tests de validación JWT + CI de seguridad.
  - 6 tests deterministas y sin red de `JwtValidator::validate_token` (válido RS256,
    `aud`, `iss`, `exp`, HS256, `kid` desconocido). Costura de test offline
    (`refresh_on_miss` + `#[cfg(test)] with_keys`), firmas públicas intactas.
  - `.github/workflows/security.yml` (`cargo audit` + `npm audit --audit-level=high`,
    push/PR a `main`/`development` + schedule semanal; ignore documentado de
    RUSTSEC-2026-0459 en `backend/.cargo/audit.toml`). **CI verde en el PR.**
  - Auditado por `@rust-reviewer`: **APROBADO**. `just check` limpio; 272+9+45+8+3 = 337 verdes.
  - Docs: `docs/jwt-validation-tests.md`, `docs/security-ci.md`. Plan `plans/PLAN-009-ARCHIVED.md`.
  - Change OpenSpec archivado: `openspec/changes/archive/2026-11-14-jwt-tests-security-ci/`
    (spec `auth` creada, `deployment` actualizada).
- Producción en **v0.15.3** (imagen `ghcr.io/atareao/watchbeat:latest`, revision `147a7bcf`),
  `healthy`, 7 monitores `up`. Despliegue gestionado por Dockge.

## Pendiente

### Issues abiertos
- **#64** 🐛 recursión sin límite en `validate_token` si el IdP devuelve JWKS vacío
  (`{"keys":[]}`) — detectado en la auditoría de #61, preexistente (**siguiente**).
- **#62** ✨ Follow-ups Robust Check Engine (canario, breaker, SSE, agrupación).
- **#63** ♿ a11y del timeline + test SNI real.

### Pendiente inmediato
- Empezar por **#64** en sesión nueva (change OpenSpec + TDD). Después **#62** y **#63**,
  cada uno en sesión separada.

### Follow-ups técnicos
- Canario de red configurable (hoy fijo a 30 s).
- Estado del breaker (`closed`/`open`) expuesto en API/dashboard.
- Dedupe de consultas DNS en vuelo.
- `error_kind` en el evento SSE (hoy se rellena con refetch).
- a11y del timeline (barras distinguibles solo por color).
- Agrupar `latency`/`expiry` en el breaker.
- SNI real no assertado con handshake TLS completo.
- `fetch_jwks` ignora el header `kid` y prueba todas las claves (ver `docs/jwt-validation-tests.md`).

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
cd backend && cargo test   # 272 + 9 + 45 + 8 + 3
cd frontend && npx tsc --noEmit && npm run build
cd backend && cargo audit && (cd ../frontend && npm audit --audit-level=high)
```