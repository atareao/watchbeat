## ADDED Requirements

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