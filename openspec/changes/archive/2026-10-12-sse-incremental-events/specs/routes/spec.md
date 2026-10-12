# Routes Module — Spec Delta: resync ante Lagged

## ADDED Requirements

### Requirement: El sse_handler SHALL manejar RecvError::Lagged

The sse_handler SHALL emit a `{"type":"resync"}` event when the broadcast stream yields `Err(RecvError::Lagged(_))`, and SHALL forward normal `Ok(data)` events unchanged.
When the `BroadcastStream` yields `Err(RecvError::Lagged(_))`, the handler emits a `{"type":"resync"}` event instead of an empty event. This signals the frontend that events were dropped and it should re-fetch current data immediately. Normal `Ok(data)` events are forwarded unchanged. Auth behavior is unchanged (`?token=` or `Authorization: Bearer`).

#### Scenario: Suscriptor lento recibe resync
- **Given** a connected SSE client whose broadcast buffer overflows
- **When** the stream yields `RecvError::Lagged(n)`
- **Then** the server sends a `{"type":"resync"}` event
- **And** the frontend triggers an immediate refresh of its data

#### Scenario: Stream normal
- **Given** a connected SSE client with no buffer overflow
- **When** check events arrive via the broadcast channel
- **Then** they are forwarded unchanged as SSE events with `type:"check"`
