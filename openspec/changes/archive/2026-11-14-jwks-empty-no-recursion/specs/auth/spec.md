## ADDED Requirements

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
