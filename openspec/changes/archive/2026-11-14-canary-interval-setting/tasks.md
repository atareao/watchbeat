To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: tests de `interval_from_setting` (default, válido, inválido, clamp min/max)
- [x] GREEN: constantes (`SETTING_CANARY_INTERVAL_SECS`, rangos) + `interval_from_setting`
- [x] RED: round-trip del setting en la BD (`set_setting`/`get_setting`)
- [x] GREEN: resolver el intervalo desde la BD en `main.rs`
- [x] RED/GREEN: `NetworkHealthHandle` aplica el nuevo intervalo en caliente
- [x] RED/GREEN: escenario "adopción en caliente" (test determinista con tokio `start_paused`)
- [x] GREEN: `routes/settings.rs` recarga el canario al cambiar `canary_interval_secs`
- [x] GREEN: `routes/export_import.rs` re-aplica el intervalo si el import lo trae
- [x] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde (355)

## Frontend

- [x] Control en `Settings.tsx` (pestaña Red) para ver/guardar el intervalo
- [x] `npx tsc --noEmit` y `npm run build` limpios

## Cierre

- [x] `docs/canary-interval-setting.md`
- [x] `openspec archive canary-interval-setting`
- [x] PR a `development` con `Closes #69`