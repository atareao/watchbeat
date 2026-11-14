## ADDED Requirements

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
