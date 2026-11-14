To check an item, replace `[ ]` with `[x]`.

## Spec

- [x] `openspec validate breaker-state --strict` OK
- [x] Aprobación explícita del usuario

## TDD

- [x] RED: test del endpoint `GET /api/breaker` (200 con `state:"closed"` + config; `"open"` tras forzar Open; 401 sin token)
- [x] GREEN: compartir el breaker desde `SchedulerManager` (`pub fn breaker()`), `manager_loop` usa el handle compartido
- [x] GREEN: `BreakerStatus` + ruta `GET /api/breaker` en el router `protected`
- [x] REFACTOR: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, suite verde

## Frontend

- [x] Tipo `BreakerStatus` + `fetchBreaker()` en `api/http.ts`
- [x] Componente `BreakerBadge.tsx`
- [x] `Dashboard.tsx` consume y muestra el badge; `MonitorCard` marca notificaciones suprimidas si está open
- [x] `npx tsc --noEmit` y `npm run build` limpios

## Cierre

- [x] `docs/breaker-state.md`
- [x] `openspec archive breaker-state`
- [x] PR a `development` con `Closes #71`
