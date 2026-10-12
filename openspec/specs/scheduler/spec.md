# Scheduler Module

## Purpose

The scheduler module (`backend/src/scheduler.rs`) manages per-monitor async tasks that periodically check monitor targets, persist results to SQLite, emit SSE events for relevant state changes, and dispatch notifications on status transitions and latency breaches.

## Requirements

### Requirement: Monitor scheduling SHALL use per-monitor tokio timers

Each monitor SHALL have its own tokio async task with a `tokio::time::interval` configured with `Duration::from_secs(interval_seconds)` and `MissedTickBehavior::Skip`. On each tick, the task SHALL execute the check probe via the appropriate checker (HTTP, TCP, Ping, TLS).

#### Scenario: Default check execution
- **Given** a monitor with `interval_seconds = 60`
- **When** the scheduler starts
- **Then** the monitor task runs the check probe every 60 seconds

### Requirement: DB writes SHALL use time-based sampling

A check SHALL be persisted to the `checks` table when the status changes OR when `(now.timestamp() - last_write_at) >= interval_secs` has elapsed. This ensures historical views reflect the configured check interval even when the status is stable. `last_write_at` SHALL be updated to `now.timestamp()` after each successful DB write.

#### Scenario: Monitor at 60s interval, always UP
- **Given** a monitor with `interval_seconds = 60`
- **When** it runs for 10 minutes (10 checks)
- **Then** 10 checks are written to DB (one per interval)
- **And** the historical view shows 60-second granularity

#### Scenario: Status change during sampling window
- **Given** a monitor with `interval_seconds = 60`
- **When** a check transitions from UP to DOWN
- **Then** the check is written immediately (status_changed = true)
- **And** `last_write_at` is updated to the current timestamp

### Requirement: Confirmation logic SHALL prevent false positives

If `confirmations_required > 0` and a check fails, the scheduler SHALL increment `failed_checks` in DB. It SHALL transition to `"down"` status only after `confirmations_required` consecutive failures. Intermediate failures SHALL be stored as `"error"` status.

#### Scenario: Confirmation required
- **Given** a monitor with `confirmations_required = 3`
- **When** 3 consecutive checks fail
- **Then** first 2 failures are stored as `"error"` status
- **And** the 3rd failure is stored as `"down"` status
- **And** a notification is dispatched

### Requirement: Panic recovery SHALL restart failed monitor tasks

A monitor task SHALL wrap the inner check loop in `AssertUnwindSafe` + `.catch_unwind()`. On panic, the error SHALL be logged with the monitor name, and the task SHALL restart after 30 seconds.

#### Scenario: Panic recovery
- **Given** a monitor task that panics
- **When** the panic is caught by `catch_unwind`
- **Then** the error is logged with monitor name
- **And** the task restarts after 30 seconds

### Requirement: Notifications SHALL dispatch on status transitions

Notifications SHALL be dispatched on status transitions (up→down, down→up), latency threshold breaches, and TLS certificate expiry. The scheduler SHALL use cached `notifier_ids` from DB (loaded once at task start) and SHALL fall back to `monitor.notifier_id` if the cached list is empty.

#### Scenario: Down notification on status change
- **Given** a monitor in UP state
- **When** a check transitions to DOWN
- **Then** a notification with type `"down"` is dispatched

### Requirement: Emisión SSE SHALL emitir solo eventos relevantes

The scheduler SHALL emit an SSE event only when the check outcome is a status change, a latency threshold breach, or the first check (state initialization). A stable monitor (state unchanged, latency within threshold) SHALL emit no subsequent SSE events. The `event_tx.receiver_count() > 0` guard SHALL be preserved so no JSON is allocated when no frontend is connected.

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

#### Scenario: Sin frontend conectado no se serializa evento
- **Given** no SSE clients connected (`receiver_count() == 0`)
- **When** a check completes
- **Then** no JSON event is serialized or sent

### Requirement: El scheduler SHALL mantener la guarda de suscriptores

The scheduler SHALL preserve the `event_tx.receiver_count() > 0` guard and SHALL keep the payload format unchanged (`{type, monitor_id, monitor_name, status, response_time_ms, error_message, checked_at}`).

#### Scenario: Payload format unchanged
- **Given** an SSE event is about to be emitted
- **When** the check outcome is status_changed or latency breach
- **Then** the JSON payload matches the established format
