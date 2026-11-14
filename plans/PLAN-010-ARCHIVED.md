# PLAN-010 — JWKS vacío sin recursión en validate_token (#64)

**Issue:** [#64](https://github.com/atareao/watchbeat/issues/64)
**Estado:** en curso
**Creado:** 2026-11-14

## Objetivo

Eliminar la recursión sin límite de `JwtValidator::validate_token` (`backend/src/auth.rs`)
cuando el endpoint JWKS responde `{"keys": []}`: hoy `fetch_jwks` devuelve `Ok` dejando la
caché vacía y `validate_token` vuelve a recursar contra el IdP indefinidamente.

## Alcance

1. Sustituir la recursión por una resolución acotada: como máximo una llamada a `fetch_jwks`;
   si la caché sigue vacía, devolver `Err`.
2. Test offline con servidor HTTP local que sirve `{"keys":[]}` y cuenta peticiones:
   `Err` y exactamente una petición.
3. Tests adicionales: fallo de red propaga `Err`; caché vacía con refresco deshabilitado → `Err` sin red.

## Tareas

- [ ] RED: test del servidor local `{"keys":[]}` → `Err` + 1 petición (falla)
- [ ] GREEN: eliminar recursión, fetch único y `Err` si la caché queda vacía
- [ ] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde
- [ ] Doc `docs/jwks-empty-no-recursion.md`
- [ ] `openspec archive jwks-empty-no-recursion`
- [ ] PR a `development` con `Closes #64`
- [ ] Renombrar a `plans/PLAN-010-ARCHIVED.md` al cerrar

## Criterios de aceptación

- Sin recursión ilimitada ante `{"keys": []}`.
- Test offline que lo verifique (determinista, sin red externa).
- `cargo clippy -- -D warnings` y `cargo fmt --check` limpios.
