# Tests de validación JWT

Cobertura de test de `JwtValidator::validate_token` (`backend/src/auth.rs`), inexistente
antes del issue #61 tras el bump `jsonwebtoken` 11 → 12 (#59). Los tests son deterministas,
sin red y sin secretos versionados.

## Qué se valida

`validate_token` acepta un token únicamente si su firma RS256 es válida con una clave JWKS
conocida, el `iss` coincide con el issuer configurado, el `aud` incluye el client id
configurado y no está expirado. Seis escenarios cubiertos:

| Escenario | Resultado esperado |
|---|---|
| Token válido RS256 (kid conocido, iss/aud correctos, exp futuro) | `Ok(Claims)` |
| `aud` incorrecta | `Err` |
| `iss` incorrecto | `Err` |
| Token expirado | `Err` |
| Algoritmo HS256 | `Err` |
| `kid` desconocido (clave ausente del JWKS) | `Err` sin red |

## Ficheros implicados

- `backend/src/auth.rs` — costura de test + `#[cfg(test)] mod tests`.
- `backend/Cargo.toml` — `rsa = "0.10"` en `[dev-dependencies]` (ya presente en el grafo vía
  `jsonwebtoken/rust_crypto`, no añade crates nuevos).

## Cómo funciona

### Costura de test offline

`JwtValidator` gana un campo interno `refresh_on_miss: bool` (por defecto `true`, igual que
antes) y un constructor `#[cfg(test)] pub(crate) fn with_keys(issuer, client_id, keys)` que
precarga las claves JWKS y pone `refresh_on_miss = false`. En `validate_token`, el fallback
de refresco por rotación solo se ejecuta si `refresh_on_miss` es true. Las firmas públicas de
`new` y `validate_token` no cambian.

La costura está bajo `#[cfg(test)]`, así que el binario de producción la compila fuera:
es imposible habilitarla accidentalmente en runtime.

### Claves y firma en el test

Dos pares RSA-2048 se generan una vez por proceso con `StdRng::seed_from_u64` (`LazyLock`):
`KEY_A` se carga en el JWKS del validador, `KEY_B` se usa como "clave desconocida". Los tokens
se firman RS256 con `EncodingKey::from_rsa_der` sobre la clave en **PKCS#1 DER** (jsonwebtoken 12
firma vía `RsaPrivateKey::from_pkcs1_der`). La verificación replica la ruta de producción con
`DecodingKey::from_rsa_raw_components(n, e)`.

El issuer de test (`issuer.test`) no resuelve, de modo que, si el refresh estuviera activo,
el error sería `failed to fetch JWKS`: la garantía de "sin red" es observable.

### Ventana de expiración

`Validation::new(RS256)` trae `leeway = 60 s`; el token expirado usa `now - 3600` y el válido
`now + 3600`, muy lejos del margen → sin flakiness.

## Decisiones

- `rsa` como dev-dependency en vez de fixture PEM en disco: determinista, reproducible y sin
  secretos versionados.
- Endurecimiento del test HS256: firma con secreto derivado de la clave pública (material `n‖e`
del JWKS), ataque clásico de alg-confusion, para atribuir el `Err` al pinning de RS256 y no a
un motivo espurio.

## Trampa a documentar para futuros refactors

En jsonwebtoken 12, borrar `set_audience` **no** hace fallar el test de audiencia, sino el happy
path (si el token trae `aud` y no hay audiencia configurada, devuelve `InvalidAudience`). La
 cobertura del check de `aud` la aporta `validation.validate_aud = false`.

## Follow-ups

- Recursión sin límite si el IdP devuelve `{"keys":[]}` → issue #64 (preexistente, fuera del
  alcance de #61).
- `fetch_jwks` ignora el header `kid` y prueba todas las claves; el escenario "kid desconocido"
es realmente "firmado por una clave ausente del JWKS".