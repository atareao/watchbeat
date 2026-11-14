# Feature: Estado del breaker (closed/open) en API y dashboard

- **Issue:** [#71](https://github.com/atareao/watchbeat/issues/71) (sub-issue bloque `d` de #62)
- **Plan:** `plans/PLAN-014.md`
- **Change OpenSpec:** `breaker-state` (archivado en `openspec/specs/breaker/spec.md`)

## Qué es

Expone el estado del **storm breaker** anti-tormenta (`closed`/`open`) y su
configuración vigente en un endpoint protegido, y lo refleja en el dashboard:
un badge global y, mientras está abierto, un indicador en cada tarjeta de monitor
avisando de que las notificaciones individuales están suprimidas.

## Ficheros implicados

- `backend/src/scheduler.rs` — `SchedulerManager` guarda el breaker
  (`Arc<Mutex<StormBreaker>>`, creado en `spawn()` con `BreakerConfig::load(&db)`) y
  lo expone con `pub fn breaker()`; `manager_loop`/`monitor_task` comparten ese mismo
  handle (antes lo creaba el propio loop).
- `backend/src/routes/breaker.rs` — `BreakerStatus` + `get_breaker` →
  `{"status":{state,enabled,threshold_percent,window_seconds,min_monitors}}`.
- `backend/src/routes/mod.rs` — ruta `GET /api/breaker` en el router **`protected`**.
- `backend/tests/breaker_route.rs` — tests del endpoint (closed+config, config no-default
  vía recarga, open, 401 sin token).
- `frontend/src/api/http.ts` — tipo `BreakerStatus` + `fetchBreaker()`.
- `frontend/src/components/BreakerBadge.tsx` — badge del estado.
- `frontend/src/components/MonitorCard.tsx` — indicador «Suprimido» si el breaker está abierto.
- `frontend/src/pages/Dashboard.tsx` — consumo y derivación de `breakerOpen`.

## Cómo funciona

1. El breaker es **único y global** (no hay uno por monitor; ver ADR 0005). Se crea
   una sola vez con `BreakerConfig::load(&db)` y su `Arc` se comparte con el scheduler;
   la recarga en caliente (`SchedulerCommand::ReloadBreaker`) llama `configure` sobre el
   mismo mutex, así que el endpoint siempre refleja la config vigente.
2. `GET /api/breaker` (requiere Bearer; no está en la whitelist de `require_auth`)
   devuelve `state` (`BreakerState::as_str()` → `"closed"|"open"`) y la config.
3. El dashboard pide el breaker junto a los monitores en un único `Promise.all`
   (`fetchBreaker().catch(() => undefined)`); deriva `breakerOpen = enabled && state === 'open'`
   en render y pinta el badge + el indicador por tarjeta. Si el fetch falla, degrada a
   «desconocido» sin romper la vista.

## Configuración

Settings ya existentes (sin cambios): `storm_breaker_enabled`, `storm_threshold_percent`,
`storm_window_seconds`, `storm_min_monitors`. El estado **no se persiste** (arranca `Closed`).
Sin variables de entorno nuevas ni migraciones.

## Cómo habilitar / desplegar

No requiere pasos: viaja con el build normal. Docker reconstruye el SPA en su stage.

## Decisiones

- **Estado global**: el breaker es único (ADR 0005); «por monitor» se interpreta como
  «visible en las vistas de monitor».
- El breaker se comparte **vía `SchedulerManager`** en lugar de duplicar un handle en `AppState`.
- Endpoint **protegido** (nunca público).
- **Degradación no bloqueante** en la UI si el estado no puede obtenerse.

## Comportamiento conocido / limitaciones

- **Rearme perezoso:** el breaker sólo reevalúa su cierre (`maybe_close`) en `on_down`/`on_up`.
  Si cesan las transiciones (p. ej. se deshabilitan los monitores caídos), `GET /api/breaker`
  puede seguir reportando `open` aunque ya no suprima nada. No incumple la spec («estado real
  del breaker»); alternativa futura: un `snapshot(now)` que rearme al leer.
- El estado se refresca con la cadencia del dashboard (evento SSE `check` + poll de 60 s).

## Follow-ups

- Evaluar `snapshot(now)` que rearme al leer para evitar `open` obsoleto.
- Higiene relacionada: `frontend/dist/index.html` está trackeado pese a `dist/` en `.gitignore`;
  la receta `just check-spec` citada en AGENTS.md no existe (la puerta SDD es `openspec validate`).
