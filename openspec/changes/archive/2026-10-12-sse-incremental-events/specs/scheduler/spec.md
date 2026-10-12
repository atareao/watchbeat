# Scheduler Module — Spec Delta: Emisión SSE incremental + resync

## ADDED Requirements

### Requirement: Emisión SSE SHALL emitir solo eventos relevantes

The scheduler SHALL emit an SSE event only when the check outcome is a status change, a latency threshold breach, or the first check (state initialization).
Emit an SSE event only when the check outcome is relevant to the frontend: a status change, a latency threshold breach, or the first check (state initialization). A stable monitor (state unchanged, latency within threshold) emits no subsequent SSE events.

#### Scenario: Monitor estable no emite evento SSE
- **Given** a monitor at a 60s interval, always UP, latency within threshold
- **When** 3 consecutive checks complete with no status change
- **Then** the first check emits an SSE event (state initialization)
- **And** the 2nd and 3rd stable checks emit no SSE event

#### Scenario: Cambio de estado dispara evento SSE
- **Given** a monitor at a 60s interval that transitions from UP to DOWN
- **When** the status-changed check completes
- **Then** an SSE event with the new status is emitted

#### Scenario: Breach de latencia dispara evento SSE
- **Given** a monitor with `latency_threshold_ms` configured
- **When** a check's response time exceeds the threshold
- **Then** an SSE event is emitted even if the status is unchanged

### Requirement: El scheduler SHALL mantener la guarda de suscriptores

The scheduler SHALL preserve the `event_tx.receiver_count() > 0` guard and SHALL keep the payload format unchanged.
Preserve the `event_tx.receiver_count() > 0` guard so no JSON is allocated when no frontend is connected. The payload format is unchanged.

#### Scenario: Sin frontend conectado no se serializa evento
- **Given** no SSE clients connected (`receiver_count() == 0`)
- **When** a check completes
- **Then** no JSON event is serialized or sent
