# PLAN-008 · Actualización de dependencias vulnerables (#59)

- **Issue**: [#59](https://github.com/atareao/watchbeat/issues/59)
- **Rama**: `feature/security-deps-update`
- **PR**: [#60](https://github.com/atareao/watchbeat/pull/60)

## Objetivo

Eliminar las vulnerabilidades de `cargo audit` y alinear el frontend con los
lockfiles ya parcheados.

## Tareas

- [x] `jsonwebtoken` 11 → 12 (`rsa` 0.10, `crypto-bigint` 0.7, `ctutils` 0.4.8).
- [x] `rustls-webpki` 0.103.17; `webpki-roots`/`webpki-root-certs` 1.0.12.
- [x] `vite ^8.3.8` en `frontend/package.json` + lock regenerado.
- [x] `cargo test`, clippy, fmt, `npm audit`, `tsc`, build.
- [x] Doc `docs/security-deps.md` y auditoría del PR (APROBADO).
- [ ] Merge a `development`, cierre de #59 y de las alertas Dependabot stale.

## Follow-ups

- `libsqlite3-sys` 0.37: bloqueado por `sqlx` 0.9.0 (RUSTSEC-2026-0459).
- Añadir tests de validación JWT (gap preexistente, ahora más relevante).
- Warnings `unsound` de `rustix` y `tokio` (no son vulnerabilidades).
