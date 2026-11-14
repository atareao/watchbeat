## ADDED Requirements

### Requirement: Test TLS con SNI real y determinista

La suite SHALL incluir un test que levante un servidor TLS local con un certificado de test para un
hostname concreto y verifique el SNI recibido en el ClientHello, sin depender de red externa.

#### Scenario: El SNI recibido coincide con el hostname del monitor
Given un servidor TLS local con certificado para `example.test`
And un monitor TLS cuyo target es ese hostname
When se ejecuta el check
Then el servidor registra un SNI igual a `example.test`
And el check termina en `up`

#### Scenario: Ejecución repetida estable
Given el test de SNI
When se ejecuta la suite varias veces seguidas
Then el resultado es siempre el mismo y no se usa red externa
