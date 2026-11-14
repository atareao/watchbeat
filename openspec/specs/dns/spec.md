# dns Specification

## Purpose

Resolver nombres de forma propia (sin depender del resolver del sistema), con
caché TTL, fallback entre upstreams, clasificación tipada de errores y
resolución multi-dirección con fallback entre direcciones en el cliente HTTP.

## Requirements

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

### Requirement: Resolución multi-dirección con fallback

El resolvedor SHALL devolver **todas** las direcciones A/AAAA de un host,
deduplicadas y con las IPv4 antes que las IPv6. La cache SHALL almacenar el
conjunto completo bajo el mismo TTL. El cliente HTTP SHALL intentar las
direcciones en orden y SHALL reintentar con la siguiente cuando una conexión
falle, de modo que un registro obsoleto no provoque un falso `down`.

`resolve()` SHALL seguir devolviendo la primera dirección para la sonda de
clasificación y para `netcheck`.

#### Scenario: Host con dos A, una muerta y una viva
Given un host con dos registros A: una IP que rechaza conexiones y otra que responde
When se ejecuta un check HTTP
Then el check termina en `up`
And la conexión se establece con la IP que responde

#### Scenario: resolve_all devuelve todas las direcciones
Given un upstream que responde con dos direcciones
When se llama a `resolve_all`
Then el resultado contiene ambas direcciones
And no contiene duplicados

#### Scenario: La cache guarda todas las direcciones
Given un host resuelto con dos direcciones
When se vuelve a llamar a `resolve_all` dentro del TTL
Then se devuelven las dos direcciones cacheadas
And no se consulta el upstream

#### Scenario: Compatibilidad de resolve
Given un host resuelto con dos direcciones
When se llama a `resolve`
Then se devuelve la primera dirección del conjunto

#### Scenario: Host con una sola dirección
Given un host con un único registro A
When se ejecuta un check HTTP
Then el comportamiento es el mismo que antes del cambio

### Requirement: Deduplicación de consultas DNS en vuelo

Cuando varias resoluciones del mismo host normalizado se soliciten concurrentemente y no exista
una entrada de caché válida, el resolver SHALL ejecutar una única consulta a upstream y compartir
su resultado (éxito o error) entre todas las llamadas concurrentes. Los aciertos de caché SHALL
servirse sin coordinación. Hosts distintos SHALL resolverse de forma independiente.

#### Scenario: Llamadas concurrentes comparten una sola consulta

- **Given** un resolver con un upstream que responde tras un retardo y cuenta las llamadas
- **When** se lanzan N resoluciones concurrentes del mismo host sin caché
- **Then** el upstream recibe exactamente una llamada
- **And** todas las llamadas devuelven los mismos IPs

#### Scenario: El error se comparte entre llamadas concurrentes

- **Given** un upstream que siempre falla (timeout)
- **When** se lanzan N resoluciones concurrentes del mismo host sin caché
- **Then** el upstream recibe exactamente una llamada
- **And** todas las llamadas reciben el mismo error

#### Scenario: Acierto de caché no coordina

- **Given** una entrada de caché válida para el host
- **When** se lanzan N resoluciones concurrentes de ese host
- **Then** el upstream no recibe ninguna llamada

#### Scenario: Hosts distintos no se deduplican entre sí

- **Given** un upstream que responde tras un retardo
- **When** se resuelven concurrentemente dos hosts distintos sin caché
- **Then** el upstream recibe una llamada por host
