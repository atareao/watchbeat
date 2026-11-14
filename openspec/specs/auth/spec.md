# auth Specification

## Purpose
TBD - created by archiving change jwt-tests-security-ci. Update Purpose after archive.

## Requirements

### Requirement: Validación de tokens de acceso JWT

El validador `JwtValidator` SHALL aceptar un token únicamente cuando su firma sea válida con una clave JWKS conocida usando el algoritmo RS256, su `iss` coincida con el issuer configurado, su `aud` incluya el client id configurado y no esté expirado. En cualquier otro caso SHALL devolver `Err`. La resolución de claves SHALL ser testeable sin acceso a red, permitiendo precargar las claves JWKS y desactivar el refresco de claves.

#### Scenario: Token válido RS256

- **Given** un par de claves RSA generado en el test y una clave pública cargada en el validador
- **When** se valida un token RS256 firmado con esa clave, con `kid` conocido, `iss`/`aud` correctos y `exp` futuro
- **Then** `validate_token` devuelve `Ok(Claims)` con el `sub` del token

#### Scenario: Audiencia incorrecta

- **Given** el validador configurado con un `client_id` concreto
- **When** se valida un token RS256 válido pero con `aud` distinta
- **Then** `validate_token` devuelve `Err`

#### Scenario: Emisor incorrecto

- **Given** el validador configurado con un `issuer` concreto
- **When** se valida un token RS256 válido pero con `iss` distinto
- **Then** `validate_token` devuelve `Err`

#### Scenario: Token expirado

- **Given** un token RS256 válido con `exp` en el pasado
- **When** se valida
- **Then** `validate_token` devuelve `Err`

#### Scenario: Algoritmo no permitido

- **Given** un token firmado con HS256 (clave simétrica)
- **When** se valida contra el validador configurado para RS256
- **Then** `validate_token` devuelve `Err`

#### Scenario: kid desconocido

- **Given** un validador con claves JWKS precargadas que no incluyen el `kid` del token
- **When** se valida un token RS256 firmado por otra clave ausente del JWKS
- **Then** `validate_token` devuelve `Err` sin aceptar la firma y sin acceder a red

### Requirement: Resolución de claves JWKS acotada

La resolución de claves de `JwtValidator` SHALL ser acotada y no recursiva. Cuando la caché de claves esté vacía, `validate_token` SHALL realizar una única llamada de llenado a `fetch_jwks`; si tras la llamada la caché sigue vacía (por ejemplo, el endpoint responde `{"keys": []}`), SHALL devolver `Err` sin volver a intentarlo. Cuando ninguna de las claves cacheadas valide el token, SHALL permitir como máximo una segunda llamada de refresco (rotación) dentro de la misma validación; en ningún caso SHALL recursar ni repetir peticiones sin límite. Un fallo de red al refrescar SHALL propagarse como `Err`.

#### Scenario: JWKS vacío no provoca recursión

- **Given** un validador con la caché de claves vacía y el refresco habilitado
- **When** el endpoint JWKS responde `{"keys": []}` y se valida un token
- **Then** `validate_token` devuelve `Err`
- **And** el endpoint recibe exactamente una petición (sin bucle)

#### Scenario: Fallo de red al refrescar propaga error

- **Given** un validador con la caché de claves vacía y el refresco habilitado
- **When** el endpoint JWKS no es alcanzable
- **Then** `validate_token` devuelve `Err` con la causa del fallo de fetch

#### Scenario: Caché vacía con refresco deshabilitado

- **Given** un validador cuyo refresco está deshabilitado
- **When** se valida un token con la caché de claves vacía
- **Then** `validate_token` devuelve `Err` sin acceder a red
