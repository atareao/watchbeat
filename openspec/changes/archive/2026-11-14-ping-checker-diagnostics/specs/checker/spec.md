## ADDED Requirements

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
