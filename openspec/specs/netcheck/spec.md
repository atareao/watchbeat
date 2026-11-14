# netcheck Specification

## Purpose
TBD - created by archiving change network-attribution. Update Purpose after archive.

## Requirements

### Requirement: Canario de salud de red (NetworkHealth)

El sistema SHALL medir periódicamente la salud de su propia conectividad de
salida mediante sondeos que **no dependan del DNS del sistema**: conexiones TCP
**por IP** a servidores DNS conocidos más una resolución DNS de un nombre conocido
a través del resolver propio. La salud SHALL tomar uno de cuatro valores:
`unknown`, `healthy`, `dns_degraded`, `down`.

#### Scenario: Salida completamente sana
Given que los sondeos TCP por IP responden
And la resolución DNS del nombre conocido funciona
When se ejecuta un ciclo del canario
Then la salud es `healthy`

#### Scenario: DNS de salida degradado
Given que los sondeos TCP por IP responden
And la resolución DNS del nombre conocido falla
When se ejecuta un ciclo del canario
Then la salud es `dns_degraded`

#### Scenario: Salida caída
Given que ningún sondeo TCP por IP responde
When se ejecuta un ciclo del canario
Then la salud es `down`

#### Scenario: Estado inicial sin información
Given que el canario todavía no ha ejecutado ningún ciclo
When se consulta la salud
Then la salud es `unknown`

#### Scenario: Los sondeos por IP no usan el DNS del sistema
Given que el DNS del sistema está caído
And la red de salida por IP funciona
When se ejecuta un ciclo del canario
Then los sondeos TCP por IP se realizan igualmente
And la salud no es `down` por culpa del DNS del sistema

#### Scenario: Un sondeo que expira cuenta como fallo
Given un sondeo que no completa dentro de su timeout
When se ejecuta un ciclo del canario
Then ese sondeo se considera fallido

### Requirement: Publicación compartida de la salud

La salud de red SHALL publicarse en un handle compartido y clonable, legible por
el scheduler y el clasificador, y SHALL actualizarse en cada ciclo del canario.
La lectura del valor actual NO SHALL bloquear.

#### Scenario: Lectura no bloqueante del último valor
Given un canario en ejecución que acaba de publicar `healthy`
When el scheduler consulta la salud
Then obtiene `healthy` sin bloquearse

#### Scenario: La salud se actualiza en cada ciclo
Given una salud publicada `healthy`
When un ciclo posterior detecta la red caída
Then la salud consultada pasa a ser `down`

### Requirement: El canario SHALL recuperarse de un ciclo con panic

Un panic dentro de un ciclo del canario NO SHALL detener el task: la salud SHALL
seguir publicándose en los ciclos posteriores. Un canario muerto no SHALL dejar la
salud congelada en un valor que impida la atribución.

#### Scenario: Un ciclo con panic no detiene el canario
Given un ciclo del canario que entra en panic
When transcurre el siguiente intervalo
Then el canario vuelve a ejecutar y publicar la salud

### Requirement: El intervalo del canario SHALL ser válido

El intervalo del canario SHALL ser estrictamente mayor que cero; construir el
canario con un intervalo nulo NO SHALL producir un panic no controlado.

#### Scenario: Intervalo no nulo
Given un intervalo de sondeo mayor que cero
When se arranca el canario
Then el canario se ejecuta sin panic

### Requirement: Intervalo del canario configurable

El periodo del canario SHALL resolverse desde el setting `canary_interval_secs`.
Cuando el setting esté ausente o no sea un entero válido, el intervalo SHALL ser
el default de 30 segundos. Un valor fuera del rango soportado SHALL clamparse a
[5, 3600] segundos.

#### Scenario: Sin setting se usa el default
- **Given** la base de datos sin el setting `canary_interval_secs`
- **When** se resuelve el intervalo del canario
- **Then** el intervalo es 30 segundos

#### Scenario: Valor válido se aplica
- **Given** el setting `canary_interval_secs` con valor `60`
- **When** se resuelve el intervalo
- **Then** el intervalo es 60 segundos

#### Scenario: Valor inválido cae al default
- **Given** el setting `canary_interval_secs` con valor `abc` o vacío
- **When** se resuelve el intervalo
- **Then** el intervalo es el default de 30 segundos

#### Scenario: Valor fuera de rango se clampea
- **Given** el setting `canary_interval_secs` con valor `0`
- **When** se resuelve el intervalo
- **Then** el intervalo se clampea al mínimo de 5 segundos
- **And** con valor `99999` el intervalo se clampea al máximo de 3600 segundos

#### Scenario: Cambio del setting se aplica en caliente
- **Given** un canario en ejecución
- **When** se guarda `canary_interval_secs` con un valor válido
- **Then** el canario adopta el nuevo periodo sin reiniciar el proceso
