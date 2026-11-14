To check an item, replace `[ ]` with `[x]`.

## Spec

- [x] `openspec validate sse-error-kind` OK
- [x] Aprobación explícita del usuario

## TDD

- [x] RED: test unitario del payload SSE (caso con causa → wire value; caso sano → `null`)
- [x] GREEN: extraer `check_event_payload(monitor, check)` (+ `error_kind`) y usarlo en `run_monitor_check`
- [x] GREEN: preservar la guarda `receiver_count() > 0` y las condiciones de relevancia
- [x] REFACTOR: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, suite completa verde

## Frontend

- [x] `CheckEvent.error_kind: string | null` en `hooks/useSse.ts`
- [x] `MonitorDetail.tsx` consume `ev.error_kind` y elimina el refetch del último check en el camino SSE
- [x] `npx tsc --noEmit` y `npm run build` limpios

## Cierre

- [x] `docs/sse-error-kind.md`
- [x] `openspec archive sse-error-kind`
- [x] PR a `development` con `Closes #70`
