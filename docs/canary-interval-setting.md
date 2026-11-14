# Canario de red configurable (`canary_interval_secs`)

Feature del issue [#69](https://github.com/atareao/watchbeat/issues/69) (sub-issue `b` de #62).

## Qué es

El **canario de red** (`netcheck`) sondea el egress propio (TCP a IPs literales + una
resolución DNS) para decidir si un fallo es del objetivo o nuestro. Su periodo estaba
**fijo en código a 30 s**; ahora es configurable vía el setting `canary_interval_secs`,
aplicable al arrancar y **en caliente**.

## Ficheros implicados

| Fichero | Papel |
|---|---|
| `backend/src/netcheck.rs` | Constantes del setting/rango, `interval_from_setting`, `NetworkHealthHandle::{set_interval,interval}`, `spawn_canary` con intervalo dinámico |
| `backend/src/main.rs` | Resuelve el setting desde la BD al arrancar |
| `backend/src/routes/settings.rs` | Recarga en caliente al persistir el setting |
| `backend/src/routes/export_import.rs` | Re-aplica el intervalo si el import lo trae |
| `frontend/src/pages/Settings.tsx` | Pestaña "Red": ver/guardar el intervalo |

## Cómo funciona

- El setting es una fila **clave-valor** en la tabla `settings` (`settings(key, value)`);
  no hay migración de esquema.
- Constantes: `SETTING_CANARY_INTERVAL_SECS = "canary_interval_secs"`,
  `DEFAULT_INTERVAL_SECS = 30`, `MIN_INTERVAL_SECS = 5`, `MAX_INTERVAL_SECS = 3600`.
- `interval_from_setting(Option<&str>)`: parsea `u64`; `None`, valor vacío, no numérico
  o negativo → **30 s** (default); el resultado se **clampea** a `[5, 3600]` s.
- **Arranque**: `main.rs` lee `canary_interval_secs` (si falla la lectura, loguea un
  warning y usa el default) y arranca el canario con ese periodo.
- **En caliente**: `POST /api/settings` con `canary_interval_secs` llama
  `NetworkHealthHandle::set_interval(interval_from_setting(valor))`; el handle emite por
  un `tokio::sync::watch` y el task del canario re-arma su ticker (`interval_at(now+period,
  period)`, `MissedTickBehavior::Skip`) sin reiniciar el proceso.
- **Import**: si el JSON importado contiene `canary_interval_secs`, se aplica al canario
  tras recargar el breaker (mismo patrón de hot-reload).

## Configuración / despliegue

- No hay variable de entorno: se configura desde la UI (**Ajustes → Red**) o vía
  `POST /api/settings`.
- Sin migración: desplegar el binario basta; rollback trivial (al omitir el setting,
  se usa el default 30 s).

## Decisiones de diseño

- **D1 — Recarga en caliente**: sí, coherente con el breaker (`reload_breaker()`).
- **D2 — Rango [5, 3600] s**: el mínimo 5 s iguala `PROBE_TIMEOUT`; el máximo 1 h.
- **D3 — Default en código** (30 s), sin sembrar fila en `settings` (como el breaker).
- **D4 — Test determinista**: el test de adopción en caliente usa `tokio::time::pause`
  (`#[tokio::test(start_paused = true)]` + `advance`), sin depender del reloj de pared.

## Follow-ups

- Deuda menor: el rango `[5, 3600]` está duplicado en TS (`CANARY_MIN`/`CANARY_MAX`) y
  Rust (`MIN_INTERVAL_SECS`/`MAX_INTERVAL_SECS`). Aceptable por el split TS/Rust.
- Asimetría intencional de clamping: `set_interval` (vía setting) clampea a `[5,3600]`,
  mientras el arranque (`seed_period`) solo exige periodo > 0 para permitir tests
  sub-segundo.