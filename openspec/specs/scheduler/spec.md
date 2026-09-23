# Scheduler Module — Baseline Spec

## Overview

The scheduler module (`backend/src/scheduler.rs`) manages per-monitor async tasks that periodically check monitor targets, persist results to SQLite, emit SSE events, and dispatch notifications on status changes.

## Architecture

```
SchedulerManager (public API)
  └─ tx: mpsc::Sender<SchedulerCommand>
       └─ manager_loop (background task)
            ├─ notifier_cache: Arc<RwLock<HashMap<String, Notifier>>>
            └─ task_map: HashMap<monitor_id, JoinHandle>
                 └─ monitor_task (per monitor, with panic recovery)
                      └─ monitor_task_inner (main check loop)
                           └─ run_monitor_check (single check execution)
```

## Contracts

### `SchedulerCommand` enum
```rust
pub enum SchedulerCommand {
    Spawn(Monitor),       // Start monitoring a new monitor
    Update(Monitor),      // Update an existing monitor (abort + respawn)
    Remove(String),       // Stop monitoring by ID
    ReloadNotifiers,      // Reload notifier cache from DB
    Shutdown,             // Abort all tasks and exit
}
```

### `SchedulerManager` struct
```rust
pub struct SchedulerManager {
    pub active_tasks: Arc<AtomicU64>,   // Number of active monitor tasks
    pub last_check_at: Arc<AtomicI64>,  // Unix timestamp of last check
    tx: mpsc::Sender<SchedulerCommand>,
}
```

### `run_monitor_check` function
```rust
pub(crate) async fn run_monitor_check(
    db: &Database,
    monitor: &Monitor,
    notifier_cache: &Arc<RwLock<HashMap<String, Notifier>>>,
    event_tx: &broadcast::Sender<String>,
    last_check_at: &Arc<AtomicI64>,
    checker: Option<&dyn Checker>,
    was_up: bool,
    notifier_ids: &[String],
    last_write_at: &mut i64,       // Timestamp of last DB write
    interval_secs: u64,             // Check interval for time-based sampling
) -> bool                           // Returns new is_up status
```

## Current Behavior (As-Is)

### Check Execution
1. `monitor_task_inner` creates a `tokio::time::interval` with `Duration::from_secs(interval_secs)` and `MissedTickBehavior::Skip`.
2. On each tick, it calls `run_monitor_check` with a `last_write_at` timestamp for time-based sampling.
3. The checker (HTTP, TCP, Ping, TLS) executes the probe.

### DB Write Sampling
- **Condition**: `status_changed || (now.timestamp() - *last_write_at >= interval_secs as i64)`
- **Effect**: A check is persisted at least once per `interval_secs` when status is stable.
- **Consequence**: For a 60-second interval, writes occur every 60 seconds → historical view matches configured interval.
- **`*last_write_at`** is updated to `now.timestamp()` after each successful DB write.

### Confirmation Logic
- If `confirmations_required > 0` and check fails, increments `failed_checks` in DB.
- Only transitions to `"down"` status after `confirmations_required` consecutive failures.
- Intermediate failures are stored as `"error"` status.

### SSE Events
- JSON event sent via `broadcast::Sender` only if `receiver_count() > 0` (zero allocations when no frontend connected).

### Notifications
- Dispatched on status transitions (up→down, down→up), latency threshold breaches, and TLS certificate expiry.
- Uses cached `notifier_ids` from DB (loaded once at task start).
- Falls back to `monitor.notifier_id` if cached list is empty.

### Panic Recovery
- `monitor_task` wraps `monitor_task_inner` in `AssertUnwindSafe` + `.catch_unwind()`.
- On panic, logs error and restarts after 30 seconds.

## Scenarios

### Scenario 1: Monitor at 60s interval, always UP
- **Given** a monitor with `interval_seconds = 60`
- **When** it runs for 10 minutes (10 checks)
- **Then** 10 checks are written to DB (one per interval)
- **And** the historical view shows 60-second granularity

### Scenario 2: Monitor at 300s interval, always UP
- **Given** a monitor with `interval_seconds = 300`
- **When** it runs for 50 minutes (10 checks)
- **Then** 10 checks are written to DB (one per interval)
- **And** the historical view shows 5-minute granularity

### Scenario 3: Status change during sampling window
- **Given** a monitor with `interval_seconds = 60`
- **When** a check transitions from UP to DOWN
- **Then** the check is written immediately (status_changed = true)
- **And** `last_write_at` is updated to the current timestamp

### Scenario 4: Confirmation required
- **Given** a monitor with `confirmations_required = 3`
- **When** 3 consecutive checks fail
- **Then** first 2 failures are stored as `"error"` status
- **And** the 3rd failure is stored as `"down"` status
- **And** a notification is dispatched

### Scenario 5: Panic recovery
- **Given** a monitor task that panics
- **When** the panic is caught by `catch_unwind`
- **Then** the error is logged with monitor name
- **And** the task restarts after 30 seconds