## ADDED Requirements

### Requirement: Deduplicación de consultas DNS en vuelo

Cuando varias resoluciones del mismo host normalizado se soliciten concurrentemente y no exista
una entrada de caché válida, el resolver SHALL ejecutar una única consulta a upstream y compartir
su resultado (éxito o error) entre todas las llamadas concurrentes. Los aciertos de caché SHALL
servirse sin coordinación. Hosts distintos SHALL resolverse de forma independiente.

#### Scenario: Llamadas concurrentes comparten una sola consulta

- **Given** un resolver con un upstream que responde tras un retardo y cuenta las llamadas
- **When** se lanzan N resoluciones concurrentes del mismo host sin caché
- **Then** el upstream recibe exactamente una llamada
- **And** todas las llamadas devuelven los mismos IPs

#### Scenario: El error se comparte entre llamadas concurrentes

- **Given** un upstream que siempre falla (timeout)
- **When** se lanzan N resoluciones concurrentes del mismo host sin caché
- **Then** el upstream recibe exactamente una llamada
- **And** todas las llamadas reciben el mismo error

#### Scenario: Acierto de caché no coordina

- **Given** una entrada de caché válida para el host
- **When** se lanzan N resoluciones concurrentes de ese host
- **Then** el upstream no recibe ninguna llamada

#### Scenario: Hosts distintos no se deduplican entre sí

- **Given** un upstream que responde tras un retardo
- **When** se resuelven concurrentemente dos hosts distintos sin caché
- **Then** el upstream recibe una llamada por host
