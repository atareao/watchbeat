# Tasks — Time-based DB write sampling

## TDD Checklist

### Phase 2 — RED (write failing tests)

- [ ] **RED-1**: Write test `test_time_based_sampling_writes_once_per_interval` — verify that for a 60s-interval monitor, each check is written when simulated time advances by 60s between calls
- [ ] **RED-2**: Write test `test_time_based_sampling_skips_rapid_checks` — verify that rapid checks (sub-interval) are not written until interval_secs have elapsed
- [ ] **RED-3**: Write test `test_status_change_writes_immediately_with_time_based` — verify status changes bypass time-based sampling
- [ ] **RED-4**: Run `cargo test` — confirm new tests FAIL while existing characterization tests still PASS

### Phase 2 — GREEN (implement)

- [ ] **GREEN-1**: Change `monitor_task_inner`: replace `check_count` with `last_write_at: i64`
- [ ] **GREEN-2**: Change `run_monitor_check` signature: replace `check_count: u64` with `last_write_at: &mut i64` and `interval_secs: u64`
- [ ] **GREEN-3**: Change write condition from `check_count % 10 == 0` to `now.timestamp() - *last_write_at >= interval_secs as i64`
- [ ] **GREEN-4**: Add `*last_write_at = now.timestamp()` after successful DB write
- [ ] **GREEN-5**: Run `cargo test` — confirm ALL tests pass (new + existing)
- [ ] **GREEN-6**: Run `cargo check` — confirm no compilation errors

### Phase 2 — REFACTOR (clean)

- [ ] **REFACTOR-1**: Remove `#[allow(clippy::manual_is_multiple_of)]` from `run_monitor_check` (no longer needed)
- [ ] **REFACTOR-2**: Run `cargo clippy -- -D warnings` — confirm zero warnings
- [ ] **REFACTOR-3**: Run `cargo fmt --check` — confirm formatting is correct
- [ ] **REFACTOR-4**: Run full `cargo test` — confirm no regressions

### Archive

- [ ] **ARCHIVE-1**: Run `openspec archive scheduler-time-based-sampling` to merge delta into baseline spec