## MODIFIED Requirements

### Requirement: El scheduler SHALL mantener la guarda de suscriptores

The scheduler SHALL preserve the `event_tx.receiver_count() > 0` guard and SHALL emit a payload that includes `error_kind` alongside the established fields: `{type, monitor_id, monitor_name, status, response_time_ms, error_message, error_kind, checked_at}`. `error_kind` SHALL carry the serialized `FailureCause` wire value when the check has an attributable cause, and SHALL be `null` otherwise. The key SHALL always be present in the payload.

#### Scenario: Payload format unchanged
- **Given** an SSE event is about to be emitted
- **When** the check outcome is status_changed or latency breach
- **Then** the JSON payload carries the established fields plus `error_kind`, and the `error_kind` key is always present

#### Scenario: Payload incluye error_kind en una caída
- **Given** an SSE event is about to be emitted for a check with an attributable failure cause
- **When** the check outcome is status_changed or latency breach
- **Then** the JSON payload contains `error_kind` equal to the serialized `FailureCause` (e.g. `connect_refused`)

#### Scenario: Monitor sano emite error_kind nulo
- **Given** an SSE event is about to be emitted for a successful check with no attributable cause
- **When** the payload is serialized
- **Then** `error_kind` is `null`

#### Scenario: La guarda de suscriptores se preserva
- **Given** no SSE clients connected (`receiver_count() == 0`)
- **When** a check completes
- **Then** no JSON event is serialized or sent
