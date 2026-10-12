# Routes Module — SSE Event Stream

## Purpose

The routes module (`backend/src/routes/mod.rs`) exposes HTTP endpoints including the SSE event stream at `GET /api/events`. This endpoint authenticates clients via JWT (query param or Bearer header), subscribes to the broadcast event channel, and forwards check events to connected SSE clients.

## Requirements

### Requirement: The sse_handler SHALL manejar RecvError::Lagged

When the `BroadcastStream` yields `Err(RecvError::Lagged(_))`, the handler SHALL emit a `{"type":"resync"}` event instead of an empty event, signaling the frontend that events were dropped and it should re-fetch current data immediately. Normal `Ok(data)` events SHALL be forwarded unchanged. Auth behavior is unchanged (`?token=` or `Authorization: Bearer`).

#### Scenario: Suscriptor lento recibe resync
- **Given** a connected SSE client whose broadcast buffer overflows
- **When** the stream yields `RecvError::Lagged(n)`
- **Then** the server sends a `{"type":"resync"}` event
- **And** the frontend triggers an immediate refresh of its data

#### Scenario: Stream normal
- **Given** a connected SSE client with no buffer overflow
- **When** check events arrive via the broadcast channel
- **Then** they are forwarded unchanged as SSE events with `type:"check"`
