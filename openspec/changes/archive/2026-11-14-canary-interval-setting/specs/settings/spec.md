## ADDED Requirements

### Requirement: Opción de intervalo del canario

La UI de Ajustes SHALL permitir ver y guardar el setting `canary_interval_secs`
(segundos). El valor SHALL validarse en el rango [5, 3600] antes de guardarse.

#### Scenario: Ver y guardar el intervalo del canario
- **Given** un usuario en Ajustes → Red
- **When** introduce `120` y guarda
- **Then** se persiste `canary_interval_secs = 120` vía `POST /api/settings`
- **And** al recargar, el control muestra `120`

#### Scenario: Valor fuera de rango no se acepta
- **Given** un usuario en Ajustes → Red
- **When** introduce `0` o `99999`
- **Then** el control no permite guardar el valor fuera de [5, 3600]