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
