# checker Specification

## Purpose
TBD - created by archiving change failure-cause-taxonomy. Update Purpose after archive.

## Requirements

### Requirement: Taxonomía de la causa de fallo

El sistema SHALL clasificar el motivo de cada fallo de check en un valor tipado
(`FailureCause`), derivado del error real y sin repetir la petición.

#### Scenario: Dominio inexistente
Given un monitor cuyo host no existe (NXDOMAIN)
When se ejecuta el check
Then la causa es `dns_nxdomain`
And el status es `down`

#### Scenario: Timeout de resolución de nombres
Given un host cuya resolución no responde
When se ejecuta el check
Then la causa es `dns_timeout`
And el status es `error`

#### Scenario: Fallo del servidor DNS
Given un host cuya resolución devuelve SERVFAIL
When se ejecuta el check
Then la causa es `dns_servfail`
And el status es `error`

#### Scenario: Conexión rechazada
Given un host resoluble cuyo puerto rechaza la conexión
When se ejecuta el check
Then la causa es `connect_refused`
And el status es `down`

#### Scenario: Timeout de conexión
Given un destino que no acepta la conexión dentro del timeout
When se ejecuta el check
Then la causa es `connect_timeout`
And el status es `error`

#### Scenario: Red inalcanzable
Given un destino con la red inalcanzable
When se ejecuta el check
Then la causa es `network_unreachable`
And el status es `error`

#### Scenario: Error TLS
Given un host HTTPS cuyo handshake TLS falla
When se ejecuta el check
Then la causa es `tls_error`
And el status es `down`

#### Scenario: Status HTTP inesperado
Given un endpoint que responde con un status distinto del esperado
When se ejecuta el check
Then la causa es `http_status`
And el status es `down`

#### Scenario: Contenido no esperado
Given un endpoint con `expected_body` que no aparece en la respuesta
When se ejecuta el check
Then la causa es `body_mismatch`
And el status es `down`

#### Scenario: Check correcto sin causa
Given un check que termina en `up`
When se ejecuta el check
Then la causa es ausente
And el status es `up`

### Requirement: Mapeo único de causa a status

El sistema SHALL derivar el status observable de una única función
`status_for(cause)`, compartida por los checkers HTTP, TCP y TLS, de modo que la
misma causa no pueda producir status distintos según el tipo de monitor.
`up` significa que el target es el culpable; `error` significa que no se pudo
establecer nada, que es responsabilidad nuestra o que es ambiguo.

| Causa | Status |
|---|---|
| ausente | `up` |
| `dns_nxdomain`, `connect_refused`, `tls_error`, `http_status`, `body_mismatch` | `down` |
| `dns_timeout`, `dns_servfail`, `connect_timeout`, `network_unreachable`, `other` | `error` |

#### Scenario: Los tres checkers coinciden en el status
Given la misma causa de fallo
When la produce un checker HTTP, uno TCP y uno TLS
Then los tres devuelven el mismo status
And ese status es el que fija la tabla de `status_for`

#### Scenario: El presupuesto de timeout es único
Given un monitor HTTP con resolución de nombres previa
When el pre-resolve consume parte del timeout
Then la petición solo dispone del tiempo restante
And la duración total del check no excede el timeout del monitor

### Requirement: La causa se propaga en el resultado del check

`CheckOutcome` SHALL incluir la causa del fallo para que capas superiores
(scheduler, persistencia) puedan usarla.

#### Scenario: El resultado lleva la causa
Given un check que falla por conexión rechazada, en HTTP, TCP o TLS
When el checker devuelve el `CheckOutcome`
Then `outcome.cause` es `Some(connect_refused)`
And `outcome.status` es `down`

### Requirement: Diagnóstico del checker de ping

El checker de ping SHALL reportar la causa del fallo en `error_message`,
incluyendo el `stderr` del comando y el código de salida cuando estén
disponibles. SHALL distinguir los fallos de permisos y de binario ausente con
mensajes accionables.

#### Scenario: ping sin permisos
Given un contenedor sin `CAP_NET_RAW`
When se ejecuta un check de ping
Then el status es `error`
And `error_message` contiene "permission denied" y una referencia a `CAP_NET_RAW`

#### Scenario: binario ping ausente
Given un entorno sin el binario `ping`
When se ejecuta un check de ping
Then el status es `error`
And `error_message` indica que el binario no está disponible

#### Scenario: ping sin respuesta
Given un target que no responde
When se ejecuta un check de ping
Then el status es `down`
And `error_message` incluye el `stderr` de ping y el código de salida

#### Scenario: ping correcto
Given un target que responde
When se ejecuta un check de ping
Then el status es `up`
And `response_time_ms` es el tiempo reportado por ping (`time=` o `tiempo=`)

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
