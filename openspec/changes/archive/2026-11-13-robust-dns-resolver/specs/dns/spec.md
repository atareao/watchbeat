# DNS — Resolver propio

## ADDED Requirements

### Requirement: Resolución de nombres con caché TTL

El sistema SHALL resolver los hostnames de los monitores mediante un resolvedor
DNS propio que cachea las respuestas respetando el TTL, en lugar de delegar en
el resolvedor del sistema operativo en cada check.

#### Scenario: Resolución de un host nuevo
Given un host que no está en la caché
When se llama a `resolve(host)`
Then se consulta a los upstreams configurados
And se devuelve una dirección IP
And la entrada queda almacenada en la caché con su TTL

#### Scenario: Segunda resolución dentro del TTL no consulta upstream
Given un host ya resuelto cuya entrada sigue vigente
When se vuelve a llamar a `resolve(host)` dentro del TTL
Then se devuelve la IP cacheada
And no se realiza ninguna consulta a los upstreams

#### Scenario: Entrada expirada se resuelve de nuevo
Given una entrada de caché cuyo TTL ha expirado
When se llama a `resolve(host)`
Then se consulta de nuevo a los upstreams

### Requirement: Fallback entre múltiples upstreams

El resolvedor SHALL consultar varios servidores DNS configurados y usar los
siguientes como respaldo si uno no responde.

#### Scenario: El primer upstream no responde
Given dos o más upstreams configurados
And el primero no responde dentro del timeout
When se llama a `resolve(host)`
Then se intenta el siguiente upstream
And se devuelve una IP si alguno responde

#### Scenario: Todos los upstreams fallan
Given que ningún upstream responde
When se llama a `resolve(host)`
Then se devuelve un error de tipo `Timeout`
And no se produce un resultado positivo falso

### Requirement: Clasificación de errores de resolución

El resolvedor SHALL distinguir entre dominio inexistente, fallo del servidor DNS
y timeout, exponiendo un error tipado.

#### Scenario: Dominio inexistente
Given un host que no existe (NXDOMAIN)
When se llama a `resolve(host)`
Then se devuelve un error de tipo `Nxdomain`

#### Scenario: Fallo del servidor DNS
Given un upstream que responde SERVFAIL
When se llama a `resolve(host)`
Then se devuelve un error de tipo `Servfail`

#### Scenario: Timeout de resolución
Given upstreams que no responden dentro del timeout
When se llama a `resolve(host)`
Then se devuelve un error de tipo `Timeout`

### Requirement: Integración transparente en los checkers

Los checkers HTTP, TCP y TLS SHALL usar el resolvedor propio preservando SNI y
la cabecera `Host`, sin alterar el comportamiento observable de los checks.

#### Scenario: Check HTTPS resuelto por la caché
Given un monitor `https://ejemplo.com` cuya IP está en caché
When se ejecuta el check
Then la petición se conecta a la IP cacheada
And el SNI y la cabecera `Host` siguen siendo `ejemplo.com`
And el check se resuelve con normalidad aunque el DNS del host esté caído

#### Scenario: Check TCP usa el resolvedor propio
Given un monitor TCP con un host
When se ejecuta el check
Then la conexión usa la IP obtenida por el resolvedor propio
