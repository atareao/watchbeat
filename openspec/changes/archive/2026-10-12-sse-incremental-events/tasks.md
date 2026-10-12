# Tasks — Emisión SSE incremental + resync

## Backend

- [x] Modify `run_monitor_check` (scheduler.rs) to emit SSE only on `status_changed` or latency breach
- [x] Add unit test: stable monitor emits no SSE event after initial (Scenario 6)
- [x] Add unit test: status change emits SSE event (Scenario 7)
- [x] Add unit test: latency breach emits SSE event (Scenario 8)
- [x] Modify `sse_handler` (routes/mod.rs) to emit `{"type":"resync"}` on `RecvError::Lagged`
- [x] Add/extend test: Lagged produces resync event (routes Scenario 1)
- [x] Add/extend test: normal stream unaffected (routes Scenario 2)
- [x] Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`

## Frontend

- [x] Update `useSse.ts`: type event as `check | resync`, add backoff exponential reconnect
- [x] Add `onResync` callback support in `useSse`
- [x] Wire `resync` in `Dashboard.tsx` to immediate `refresh()`
- [x] Wire `resync` in `MonitorDetail.tsx` to immediate data reload
- [x] Verify build: `npm run build` (tsc -b && vite build)

## Cleanup

- [x] Verify fallback poll of 60s still present as safety net
- [x] Run full pre-commit: `just check`
- [x] `openspec archive sse-incremental-events`
