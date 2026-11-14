# frontend-status-contrast Specification

## Purpose
Garantizar que los colores de estado de la aplicación sean legibles: el texto sobre las superficies
de estado (tags, tarjetas, leyenda) debe cumplir contraste accesible tanto en tema claro como
oscuro.

## Requirements

### Requirement: El texto sobre superficies de estado SHALL cumplir contraste AA

El texto sobre cualquier superficie coloreada por estado (por ejemplo los `Tag` de `MonitorCard`)
SHALL alcanzar una relación de contraste de al menos 4.5:1 (texto normal) contra su fondo, tanto en
tema claro como en tema oscuro, sin perder la codificación por color del estado.

#### Scenario: Tag de estado en tema claro
Given un monitor con estado `up`, `warning`, `down`, `error`, `indeterminate` o `no_data`
When se renderiza su `Tag` en tema claro
Then el contraste texto/fondo es mayor o igual a 4.5:1

#### Scenario: Tag de estado en tema oscuro
Given un monitor con cualquiera de esos estados
When se renderiza su `Tag` en tema oscuro
Then el contraste texto/fondo es mayor o igual a 4.5:1

#### Scenario: El color del estado no se pierde
Given un `Tag` ajustado para cumplir contraste
When se renderiza
Then sigue siendo reconocible el estado por su color
