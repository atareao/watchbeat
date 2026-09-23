# Change Proposal: Time-based DB write sampling in scheduler

## Intent

Replace the current count-based sampling (`check_count % 10 == 0`) with time-based sampling in `run_monitor_check`. This ensures the historical check view accurately reflects the configured check interval.

## Problem

The scheduler persists check results to the `checks` table using a count-based modulo: `check_count % 10 == 0`. For a monitor configured at 60-second intervals, this means only 1 in 10 checks is written to the database — a write every 10 minutes. The historical view shows 10-minute gaps, making it appear that the monitor checks every 10 minutes instead of every 1 minute.

## Scope

**Single file**: `backend/src/scheduler.rs`

Changes:
- `monitor_task_inner`: Replace `check_count: u64` tracking with `last_write_at: i64` timestamp
- `run_monitor_check` signature: Replace `check_count: u64` param with `last_write_at: &mut i64` and `interval_secs: u64`
- Write condition: Change from `check_count % 10 == 0` to `now.timestamp() - last_write_at >= interval_secs`
- Update `*last_write_at` after each successful DB write

## Impact

| Metric | Before | After |
|--------|--------|-------|
| Writes/día (monitor 60s, estable) | 144 | 1,440 |
| Writes/día (20 monitores a 60s) | 2,880 | 28,800 |
| Gap máximo en histórico (60s) | 10 min | 60s |
| Gap máximo en histórico (300s) | 50 min | 5 min |

SQLite WAL mode with `synchronous=NORMAL` handles 0.33 writes/sec without issue. Storage impact: ~130 MB/month for 20 monitors at 60s (vs ~13 MB before).

## Non-goals

- No changes to SSE event emission (already per-check)
- No changes to notification dispatch logic
- No changes to the panic recovery mechanism
- No changes to the confirmation/failed_checks logic
- No changes to the database schema or migrations

## Test strategy

- Unit tests with mock checkers and temp SQLite DB
- Verify that for N consecutive stable checks, writes occur at least once per `interval_secs` of simulated time
- Verify that status changes still trigger immediate writes regardless of elapsed time