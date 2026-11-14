# Proposal

**Issue:** [#61](https://github.com/atareao/watchbeat/issues/61)
**Plan:** `plans/PLAN-009.md`

## Why

El bump de `jsonwebtoken` 11 → 12 (#59) se validó a mano: `JwtValidator::validate_token` (`backend/src/auth.rs`) no tiene tests, así que una regresión en la validación de firma, `iss`, `aud`, `exp` o en la rotación de claves JWKS pasaría desapercibida. Además Dependabot se quedó stale y no detectó las vulnerabilidades reales que sí veía `cargo audit`; no existe ninguna comprobación automatizada de seguridad en CI.

## What Changes

- Se añade una costura de test en `JwtValidator` para precargar claves JWKS y desactivar el refresco de claves, de modo que la validación sea testeable sin red.
- Se añaden tests deterministas que generan un par RSA en el propio test y firman tokens RS256: válido → `Ok`, `aud` incorrecta → `Err`, `iss` incorrecto → `Err`, expirado → `Err`, algoritmo HS256 → `Err`, `kid` desconocido → `Err`.
- Se añade `.github/workflows/security.yml` con dos jobs: `cargo audit` (Rust) y `npm audit --audit-level=high` (frontend); triggers `push`/`pull_request` a `main`/`development` y `schedule` semanal; el job falla ante vulnerabilidades, con `ignore` documentado solo para `RUSTSEC-2026-0459` mientras siga bloqueada por sqlx.

## Capabilities

### Added Capabilities

- `auth`: la validación de tokens JWT SHALL aceptar únicamente tokens RS256 con firma válida, `iss`/`aud` correctos, no expirados y `kid` conocido, y SHALL rechazar cualquier otra combinación.
- `deployment`: la CI SHALL ejecutar auditorías de dependencias (`cargo audit` y `npm audit`) en `push`/PR a `main`/`development` y semanalmente, fallando ante vulnerabilidades conocidas.

## Impact

- `backend/src/auth.rs` (costura de test + tests inline).
- `backend/Cargo.toml` (`dev-dependencies` para generar/firmar RSA en tests).
- `.github/workflows/security.yml` (nuevo).
- Sin cambios de comportamiento en producción, ni de API, ni de esquema.