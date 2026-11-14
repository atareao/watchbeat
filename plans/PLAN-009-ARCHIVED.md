# PLAN-009 — Tests de validación JWT + CI de seguridad (#61)

**Issue:** [#61](https://github.com/atareao/watchbeat/issues/61)
**Estado:** en curso
**Creado:** 2026-11-14

## Objetivo

Cubrir con tests deterministas la validación de tokens JWT (`JwtValidator::validate_token`, `backend/src/auth.rs`), hoy sin cobertura tras el bump `jsonwebtoken` 11 → 12 (#59), y añadir una CI de seguridad que detecte vulnerabilidades de dependencias en cada PR y de forma programada.

## Alcance

1. **Tests de `JwtValidator`** (par RSA generado en el test, sin red):
   token válido RS256 → Ok; `aud` incorrecta → Err; `iss` incorrecto → Err;
   expirado → Err; algoritmo HS256 → Err; `kid` desconocido → Err.
2. **`.github/workflows/security.yml`**: `cargo audit` (Rust) + `npm audit --audit-level=high` (frontend); triggers `push`/`pull_request` a `main`/`development` + `schedule` semanal;
   falla ante vulnerabilidades; `ignore` documentado solo para `RUSTSEC-2026-0459`.

## Tareas

- [ ] RED: tests de `JwtValidator` que fallan
- [ ] GREEN: costura de test offline + mínima implementación
- [ ] RED/GREEN: `security.yml` con jobs de auditoría
- [ ] REFACTOR: clippy/fmt limpios, suite completa verde
- [ ] Doc `docs/`
- [ ] PR a `development` con `Closes #61`
- [ ] `openspec archive jwt-tests-security-ci`
- [ ] Renombrar a `plans/PLAN-009-ARCHIVED.md` al cerrar

## Criterios de aceptación

- Tests deterministas, sin red externa.
- Workflow verde en el PR y fallando ante una vulnerabilidad conocida.
- `cargo clippy -- -D warnings` y `cargo fmt --check` limpios.