## ADDED Requirements

### Requirement: El checker TLS SHALL enviar el hostname como SNI

Al realizar el handshake TLS, el checker SHALL enviar como SNI (Server Name Indication) el
hostname del monitor, tomado del target textual y no de la IP resuelta, de modo que los servidores
con virtual hosting presenten el certificado correcto.

#### Scenario: Hostname textual produce el SNI correcto
Given un monitor TLS cuyo target es un hostname `example.test`
When se ejecuta el check contra un servidor TLS local que registra el SNI recibido
Then el SNI recibido es `example.test`

#### Scenario: IP literal no envía SNI
Given un monitor TLS cuyo target es una IP literal
When se ejecuta el check
Then el handshake se completa sin extensión SNI y no falla por ello
