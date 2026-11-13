To check an item, replace `[ ]` with `[x]`.

## Preparación
- [x] Issue #37 creado
- [x] Change proposal OpenSpec creado
- [x] Rama `feature/network-attribution`

## RED — Tests
- [x] Canario: sondeos OK + DNS OK → `healthy`
- [x] Canario: sondeos IP OK + DNS falla → `dns_degraded`
- [x] Canario: todos los sondeos IP fallan → `down`
- [x] Canario: estado inicial → `unknown`
- [x] Canario: un sondeo que expira cuenta como fallo
- [x] Canario: lectura del handle no bloqueante y actualizada por ciclo
- [x] Clasificador: tabla de verdad completa (up, nxdomain, healthy, unknown, dns_degraded+dns_*, dns_degraded+no-dns, down)
- [x] Clasificador: `indeterminate` ≠ `down`/`error`
- [x] Scheduler: fallo real + red sana → `down` (notifica)
- [x] Scheduler: fallo DNS + red `dns_degraded` → `indeterminate` (NO notifica, no cambia was_up)
- [x] Scheduler: `run-check` manual usa la misma atribución
- [x] DB: uptime excluye `indeterminate` (100% con 8 up + 2 indeterminate; 80% con 8 up + 2 down; sin datos si solo indeterminate)

## GREEN — Implementación
- [x] `backend/src/netcheck.rs`: `NetworkHealth`, `NetworkHealthHandle`, task del canario (probes TCP por IP + resolución vía `dns::resolve`), inyectable para tests
- [x] `backend/src/attribution.rs`: `classify(cause, health)` con la tabla de verdad
- [x] `main.rs`: arrancar el canario y pasar el handle al scheduler
- [x] `scheduler.rs`: aplicar `classify` en `run_monitor_check`; `indeterminate` neutral en notificación y `was_up`
- [x] `routes/monitors.rs` (run-check): aplicar la misma atribución
- [x] `db.rs`: uptime excluye `indeterminate` del denominador (todas las variantes de cálculo)
- [x] `lib.rs`: exportar `netcheck` y `attribution`
- [x] Frontend: tipo de estado `indeterminate` (api/http.ts, useSse.ts), badge en `MonitorCard` y timeline en `MonitorDetail`

## REFACTOR
- [x] `cargo clippy --all-targets --all-features -- -D warnings` limpio
- [x] `npx tsc --noEmit` limpio
- [x] `npm run build` OK

## Documentación
- [x] Crear `docs/network-attribution.md`

## Correcciones de auditoría
- [x] (M1) `attribute`: un fallo con `cause = None` + red `down` → `indeterminate` (cubre `ping`)
- [x] (M2) `get_timeline_buckets`: excluir `indeterminate` del denominador y emitir `dominant_status="indeterminate"` si no hay checks atribuibles
- [x] (M3) Dashboard: exponer `indeterminate_monitors` y que el total cuadre
- [x] (M4) `status_pages`: distinguir `indeterminate` (fila + estado global)
- [x] (M5) Scheduler: un corte sostenido de `indeterminate` no emite un evento SSE por check
- [x] (M6) Canario: recuperación de panic por ciclo + invariante de intervalo no nulo
- [x] (M7) Tests de secuencias de transición (`up→indeterminate→up`, `down→indeterminate→down`, primer check) y `indeterminate` + `confirmations_required`
- [x] (N2) `attribute` delega en `classify` para `Some(cause)`; test tabla-driven que recorre todas las combinaciones (cause × health) y los casos `cause = None`
- [x] (N4) Etiqueta visible de `indeterminate` en español (`Indeterminado`), sin cambiar el valor interno
- [x] (N5) El estado observado del scheduler avanza solo tras un `insert_check` correcto (la transición se reintenta si la escritura falla)
- [x] (N6) Renombrar `test_count_based_sampling_writes_every_10th_check` → muestreo por tiempo
- [x] Frontend: stat card `indeterminate` en Dashboard
- [x] Frontend: incluir `indeterminate` en el filtro «Solo errores» del detalle
- [x] Frontend: exponer y mostrar `error_kind` (tipo + columna/tooltip)

## Cierre
- [x] `cargo test` verde
- [x] PR a `development` (`Closes #37`)
- [x] `openspec archive network-attribution`
- [x] Actualizar `plans/PLAN-002.md`
