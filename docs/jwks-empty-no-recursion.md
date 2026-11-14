# JWKS vacío sin recursión en `validate_token`

Fix del issue #64 (detectado en la auditoría del PR #61). `JwtValidator::validate_token`
(`backend/src/auth.rs`) recursaba cuando la caché de claves estaba vacía; con un IdP que
responde `{"keys": []}` eso producía un bucle de peticiones sin límite (DoS y, en la práctica,
`stack overflow`).

## Qué cambia

`validate_token` deja de recursar. Ante una caché vacía:

1. Si el refresco está deshabilitado (`refresh_on_miss == false`) → `Err` inmediato, sin red.
2. Si no, una **única** llamada de llenado a `fetch_jwks`:
   - fallo de red → `Err` (propagado con la causa);
   - el endpoint devuelve `{"keys": []}` → `Err("JWKS endpoint returned an empty key set")`.
3. Si hay claves, se intentan validar. Si ninguna valida, se permite **una** llamada más de
   refresco por rotación; después, `Err`.

En total, como máximo dos peticiones por validación y **nunca** recursión ni bucle.

## Ficheros implicados

- `backend/src/auth.rs` — reescritura de `validate_token` + 3 tests en `#[cfg(test)] mod tests`.

## Cómo se verifica (tests offline, sin red externa)

- `empty_jwks_does_not_recurse`: servidor HTTP mínimo en `127.0.0.1` que sirve `{"keys":[]}` y
  cuenta conexiones; assert de `Err` (mensaje `empty key set`) y de **exactamente 1** petición.
  La llamada va envuelta en `tokio::time::timeout` para fallar fail-fast si se reintrodujera un bucle.
- `unreachable_jwks_propagates_fetch_error`: issuer a puerto cerrado (`127.0.0.1:1`) → `Err`
  con `failed to fetch JWKS` (con timeout).
- `empty_cache_with_refresh_disabled_errors_without_network`: caché vacía + refresco deshabilitado
  → `Err` con `network refresh is disabled` (con timeout).

## Por qué ocurría

`fetch_jwks` devuelve `Ok` aunque el array `keys` venga vacío (solo falla si falta el campo
`keys`). La rama de caché vacía asumía que tras el fetch siempre habría claves y volvía a llamar
a `validate_token`. Con `{"keys": []}` el supuesto se rompe y el ciclo no termina.

## Troubleshooting

- Si un token válido deja de pasar, revisar la rotación de claves del IdP: el camino de rotación
  permite una segunda petición; un `Err` con `no matching JWK found for token` indica que la
  clave no está en el JWKS ni tras el refresco.

## Follow-ups

- El `Vec<DecodingKey>` se clona por request autenticada (comportamiento preexistente; no es
  regresión de este fix).
- La costura de test `with_keys` sigue bajo `#[cfg(test)]`, no compilable en producción.
