# Proposal

**Issue:** [#64](https://github.com/atareao/watchbeat/issues/64)
**Plan:** `plans/PLAN-010.md`

## Why

En `JwtValidator::validate_token` (`backend/src/auth.rs`), si la caché de claves está vacía
se ejecuta `fetch_jwks` y **se recursa**. Un IdP que responde `{"keys": []}` hace que
`fetch_jwks` devuelva `Ok` dejando la caché vacía, de modo que la recursión se repite contra
el IdP **sin límite** (bucle de peticiones, DoS / agotamiento). Detectado en la auditoría
del PR #61, sobre código no modificado por ese PR.

## What Changes

- Se elimina la recursión de `validate_token`: con la caché vacía se realiza **un único**
  `fetch_jwks`; si el endpoint devuelve un conjunto de claves vacío, se devuelve `Err` en
  lugar de reintentar. Un fallo de red se propaga como `Err`.
- Se añade un test offline (servidor HTTP local en `127.0.0.1`, sin red externa) que sirve
  `{"keys": []}` y cuenta peticiones: verifica `Err` y exactamente una petición.
- Tests adicionales para el fallo de red y para la caché vacía con refresco deshabilitado.

## Capabilities

### Added Capabilities

- `auth`: la resolución de claves JWKS SHALL ser acotada y no recursiva; ante una caché
  vacía tras el fetch SHALL devolver `Err`.

## Impact

- `backend/src/auth.rs` (`validate_token` + `#[cfg(test)] mod tests`).
- Sin cambios de API ni de esquema.
