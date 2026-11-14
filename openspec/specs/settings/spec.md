# settings Specification

## Purpose
TBD - created by archiving change retention-14-days. Update Purpose after archive.

## Requirements

### Requirement: Opciones de retención

El Select de retención SHALL ofrecer las opciones `7 días` y `14 días`.

#### Scenario: Select muestra 7 y 14 días
Given un usuario en Ajustes → Retención
When ve el Select de retención
Then las opciones disponibles son `7 días` y `14 días`
And la opción `15 días` no aparece
And la opción `30 días` no aparece

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
