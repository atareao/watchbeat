## ADDED Requirements

### Requirement: Fallback multi-dirección en TCP y TLS

Los checkers `tcp` y `tls` SHALL resolver todas las direcciones del host y SHALL
intentar cada una en orden hasta que una conecte (TLS: hasta completar el
handshake). Si todas fallan, SHALL devolver una causa tipada y un mensaje que
resuma los intentos.

#### Scenario: TCP con dos A, una muerta y una viva
Given un host con una IP que rechaza conexiones y otra que acepta
When se ejecuta un check TCP
Then el check termina en `up`
And el mensaje no contiene errores

#### Scenario: TLS con dos A, una muerta y una viva
Given un host con una IP que rechaza conexiones y otra con TLS válido
When se ejecuta un check TLS
Then el check termina en `up`

#### Scenario: Todas las direcciones fallan
Given un host cuyas direcciones rechazan la conexión
When se ejecuta un check TCP
Then el status es `down`
And la causa es `connect_refused`
And el mensaje menciona cada intento

#### Scenario: Host con una sola dirección
Given un host con un único registro
When se ejecuta un check TCP o TLS
Then el comportamiento es el mismo que antes del cambio
