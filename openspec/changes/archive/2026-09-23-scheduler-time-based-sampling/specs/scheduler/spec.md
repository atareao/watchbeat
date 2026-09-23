# Scheduler Module — Spec Delta: Time-based sampling

### ADDED `run_monitor_check` function (new signature)
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
    last_write_at: &mut i64,       // CHANGED: timestamp instead of counter
    interval_secs: u64,             // NEW: interval for time-based sampling
) -> bool
```

### ADDED DB Write Sampling (NEW)
- **Condition**: `status_changed || (now.timestamp() - *last_write_at >= interval_secs as i64)`
- **Effect**: A check is persisted at least once per `interval_secs` when status is stable.
- **Consequence**: For a 60-second interval, writes occur every 60 seconds → historical view matches configured interval.
- **`*last_write_at`** is updated to `now.timestamp()` after each successful DB write.

### ADDED Scenario 1: Monitor at 60s interval, always UP
- **Given** a monitor with `interval_seconds = 60`
- **When** it runs for 10 minutes (10 checks)
- **Then** 10 checks are written to DB (one per interval)
- **And** the historical view shows 60-second granularity

### ADDED Scenario 2: Monitor at 300s interval, always UP
- **Given** a monitor with `interval_seconds = 300`
- **When** it runs for 50 minutes (10 checks)
- **Then** 10 checks are written to DB (one per interval)
- **And** the historical view shows 5-minute granularity

### ADDED Scenario 3: Status change during sampling window
- **Given** a monitor with `interval_seconds = 60`
- **When** a check transitions from UP to DOWN
- **Then** the check is written immediately (status_changed = true)
- **And** `last_write_at` is updated to the current timestamp