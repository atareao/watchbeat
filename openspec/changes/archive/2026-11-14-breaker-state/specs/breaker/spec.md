## ADDED Requirements

### Requirement: El estado del breaker SHALL exponerse en la API

El backend SHALL exponer el estado del storm breaker mediante un endpoint protegido (`GET /api/breaker`), devolviendo el estado (`closed`/`open`) y la configuración vigente (`enabled`, `threshold_percent`, `window_seconds`, `min_monitors`). El estado SHALL leerse del breaker real compartido con el scheduler y NO SHALL exponerse en endpoints públicos.

#### Scenario: Estado cerrado en condiciones normales
- **Given** un breaker sin episodios de tormenta activos
- **When** se llama a `GET /api/breaker`
- **Then** la respuesta contiene `state: "closed"`
- **And** incluye `enabled`, `threshold_percent`, `window_seconds` y `min_monitors`

#### Scenario: Estado abierto durante una tormenta
- **Given** un breaker que se ha abierto por una tormenta de caídas
- **When** se llama a `GET /api/breaker`
- **Then** la respuesta contiene `state: "open"`

#### Scenario: El endpoint requiere autenticación
- **Given** una petición sin token válido
- **When** se llama a `GET /api/breaker`
- **Then** la respuesta es `401`

### Requirement: El dashboard SHALL reflejar el estado del breaker

El dashboard SHALL mostrar el estado del breaker (cerrado/abierto) y, mientras esté abierto, SHALL indicar en las tarjetas de monitor que las notificaciones individuales están siendo suprimidas por una tormenta. Si el estado no puede obtenerse, la UI SHALL degradar de forma no bloqueante, sin romper la vista.

#### Scenario: Indicador acorde al estado abierto
- **Given** el dashboard cargado
- **When** el estado del breaker es `open`
- **Then** se muestra un indicador de «abierto» (tormenta)
- **And** las tarjetas de monitor indican notificaciones suprimidas

#### Scenario: Estado cerrado sin aviso
- **Given** el estado del breaker es `closed`
- **When** el dashboard se renderiza
- **Then** no se muestra ningún aviso de tormenta
