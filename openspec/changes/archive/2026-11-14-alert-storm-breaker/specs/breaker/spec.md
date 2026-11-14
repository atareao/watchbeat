# Breaker — Anti-tormenta de alertas

## ADDED Requirements

### Requirement: El breaker SHALL detectar tormentas y emitir una única alerta agregada

El sistema SHALL incorporar un *breaker* que detecte cuándo varios monitores caen dentro
de una ventana de `Y` segundos y, en ese caso, emitir **una sola** alerta agregada
("monitorización degradada") en lugar de una notificación por monitor. La tormenta SHALL
declararse cuando el número de monitores que pasan a `down` dentro de la ventana alcanza el
umbral `max(min_monitors, ceil(X % · monitores_habilitados))`. Si el número de monitores
habilitados es menor que `min_monitors`, el breaker NO SHALL declarar tormenta. La alerta
agregada SHALL incluir el recuento y la lista de monitores afectados.

#### Scenario: Muchos monitores caen a la vez → una sola alerta
Given 8 monitores habilitados y un umbral del 50 %
When 4 monitores pasan a `down` dentro de la ventana de `Y` segundos
Then se emite una única alerta agregada
And las notificaciones individuales de esas caídas no se envían

#### Scenario: Pocas caídas aisladas → notificación individual normal
Given un umbral de 4 monitores en la ventana
When sólo 2 monitores pasan a `down` dentro de la ventana
Then cada caída se notifica de forma individual
And no se emite alerta agregada

#### Scenario: Umbral absoluto mínimo
Given `min_monitors = 3` y un único monitor habilitado
When ese monitor pasa a `down`
Then NO se declara tormenta y se notifica individualmente

#### Scenario: La alerta agregada incluye recuento y lista
Given una tormenta declarada con los monitores "web", "api" y "db"
When se emite la alerta agregada
Then el mensaje incluye el recuento (3)
And el mensaje incluye los nombres de los monitores afectados

### Requirement: El breaker SHALL aplicar dedupe por ventana (una alerta por episodio)

Mientras el breaker esté en estado `Open`, NO SHALL volver a emitirse la alerta agregada
aunque sigan cayendo monitores. El breaker SHALL rearmarse (volver a `Closed`) cuando
transcurra la ventana `Y` sin nuevas caídas; sólo entonces podrá declararse una nueva
tormenta y emitirse una nueva agregada.

#### Scenario: No se repite la agregada dentro del mismo episodio
Given una tormenta ya declarada y su alerta agregada emitida
When un quinto monitor pasa a `down` dentro de la misma ventana
Then NO se emite otra alerta agregada

#### Scenario: Un episodio nuevo vuelve a alertar
Given una tormenta que se rearmó tras pasar la ventana sin nuevas caídas
When vuelven a caer monitores por encima del umbral dentro de una nueva ventana
Then se emite una nueva alerta agregada

### Requirement: El breaker SHALL suprimir las notificaciones individuales durante la tormenta

Mientras el breaker esté `Open`, las transiciones individuales `down` y `up` de los
monitores NO SHALL generar notificaciones.

#### Scenario: Una recuperación a media tormenta no notifica
Given el breaker en estado `Open`
When un monitor se recupera (`up`) antes de que la tormenta se rearme
Then no se envía una notificación individual de recuperación

#### Scenario: Tras rearmarse, la recuperación sí notifica
Given el breaker rearmado a `Closed`
When un monitor se recupera
Then se envía la notificación individual de recuperación

### Requirement: El breaker SHALL ser configurable y desactivable

El breaker SHALL leer su configuración de los settings globales: `storm_breaker_enabled`
(por defecto `true`), `storm_threshold_percent` (por defecto `50`), `storm_window_seconds`
(por defecto `10`) y `storm_min_monitors` (por defecto `3`). Cuando
`storm_breaker_enabled = false`, el breaker NO SHALL suprimir ni agregar ninguna
notificación (comportamiento clásico). Un cambio de configuración SHALL poder aplicarse en
caliente sin reiniciar el proceso.

#### Scenario: Desactivado se comporta como antes
Given `storm_breaker_enabled = false`
When muchos monitores caen a la vez
Then cada caída se notifica individualmente
And no se emite alerta agregada

#### Scenario: Un umbral del 100 % exige el 100 % de los monitores
Given `storm_threshold_percent = 100` y 8 monitores habilitados
When 3 monitores caen en la ventana
Then no se declara tormenta

#### Scenario: Reconfiguración en caliente
Given el breaker con umbral del 50 %
When se guarda `storm_threshold_percent = 80`
Then el breaker usa el nuevo umbral sin reiniciar el proceso

### Requirement: El breaker SHALL ser determinista y testeable sin esperas reales

El cálculo del breaker SHALL recibir el instante actual de forma inyectada, de modo que los
tests puedan avanzar el tiempo de forma determinista sin usar `sleep` real.

#### Scenario: Detección reproducible con reloj inyectado
Given un reloj inyectado que se avanza manualmente
When se simulan N caídas dentro y fuera de la ventana
Then el estado y las decisiones del breaker son reproducibles y no dependen de tiempo real

### Requirement: La alerta agregada SHALL usar una plantilla configurable

La alerta agregada SHALL renderizarse con la plantilla `default_template_storm` (settings) y,
en su ausencia, con una plantilla por defecto. El contexto SHALL exponer `count`,
`monitors`, `monitors_list`, `window_seconds` y `threshold_percent`.

#### Scenario: Plantilla por defecto
Given que no hay `default_template_storm` configurada
When se emite la alerta agregada
Then el mensaje se renderiza con la plantilla por defecto
And contiene el recuento y los nombres de los monitores afectados
