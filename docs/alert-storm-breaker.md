# Anti-tormenta / breaker de alertas

## Qué es

Cuando la red del host, un proveedor DNS o un tramo de red intermedio falla, **muchos
monitores caen en el mismo instante**. Sin protección, cada caída dispara su propia
notificación y una única incidencia de infraestructura genera **N alertas** (una
"tormenta") que saturan al operador y esconden la causa real.

El **breaker** detecta esa situación y emite **una sola alerta agregada** ("monitorización
degradada") en lugar de N individuales. Mientras dura la tormenta suprime las
notificaciones individuales y se rearma al pasar la ventana, de modo que **una tormenta =
una alerta**.

> Complementa a la Fase 3: el canario de red (`indeterminate`) evita los falsos `down`
> cuando la red del host está caída. El breaker gobierna el **volumen** de alertas cuando
> el canario está sano pero varios targets fallan a la vez.

## Cómo funciona

Máquina de estados pura en `backend/src/breaker.rs` (`StormBreaker`), con **reloj
inyectado** (testeable sin `sleep` reales):

- **Estados**: `Closed` (normal) y `Open` (tormenta en curso), **uno por tipo**: caídas
  (`down`/`up`), `latency` y `expiry` (ver `docs/latency-expiry-aggregation.md`).
- **Detección**: se declara tormenta cuando el número de **monitores únicos** que emiten la
  misma señal dentro de una ventana de `Y` segundos alcanza
  `max(min_monitors, ceil(X % · monitores_habilitados))`.
  - Si `monitores_habilitados < min_monitors`, **nunca** se declara tormenta (despliegues
    pequeños conservan la semántica clásica de una alerta por monitor).
  - El recuento es por **monitor único** (dedupe por `monitor_id`): un monitor que flapea
    no dispara una falsa tormenta.
- **Al abrir**: se emite **una** alerta agregada con el recuento y la lista de monitores
  afectados, a la **unión deduplicada** de los notificadores de todos los afectados.
- **Dedupe**: durante `Open` no se repite la agregada aunque sigan llegando señales.
- **Supresión**: durante `Open` se suprimen las transiciones individuales — caídas,
  recuperaciones y las señales `latency`/`expiry` del mismo tipo.
- **Rearme**: al pasar la ventana `Y` sin nuevas señales **registradas** de ese tipo, vuelve
  a `Closed`; un nuevo episodio puede volver a alertar. Con el episodio de señal abierto las
  señales suprimidas **no** se registran (si no, un monitor con `interval < ventana` lo
  mantendría abierto para siempre).

## Configuración

Claves en la tabla `settings` (KV), editables desde la pestaña **"Tormenta"** de
`Settings`:

| Clave | Default | Descripción |
|---|---|---|
| `storm_breaker_enabled` | `true` | Activa/desactiva el breaker. `false` = comportamiento clásico. |
| `storm_threshold_percent` | `50` | `X`: % de monitores habilitados que deben emitir la señal. |
| `storm_window_seconds` | `10` | `Y`: ventana de detección en segundos. |
| `storm_min_monitors` | `3` | Suelo absoluto de monitores para declarar tormenta. |
| `default_template_storm` | (vacío) | Plantilla de la alerta agregada **de caídas**. |

La configuración se aplica **en caliente**: guardar en la UI (o importar settings) emite
`SchedulerCommand::ReloadBreaker`, que reconfigura el breaker sin reiniciar.

### Plantilla de la alerta agregada

Contexto disponible en `default_template_storm`:

- `{{ count }}` — número de monitores afectados.
- `{{ monitors_list }}` — nombres separados por comas.
- `{{ monitors }}` — array de nombres.
- `{{ window_seconds }}` — ventana `Y`.
- `{{ threshold_percent }}` — umbral `X`.

Si no se configura, se usa la plantilla por defecto `template::defaults::STORM`. Las
agregadas de `latency`/`expiry` usan `defaults::STORM_LATENCY`/`defaults::STORM_EXPIRY` con
el mismo contexto (ver `docs/latency-expiry-aggregation.md`).

## Ficheros implicados

- `backend/src/breaker.rs` — máquina de estados (`Episode` por tipo), `SignalKind`,
  `on_signal`, config, decisión, tests.
- `backend/src/scheduler.rs` — `Arc<Mutex<StormBreaker>>` compartido, punto de decisión en
  el despacho (`down`/`up` y señales `latency`/`expiry`), `ReloadBreaker`, unión de
  notificadores.
- `backend/src/template.rs` — `defaults::STORM`, `defaults::STORM_LATENCY`,
  `defaults::STORM_EXPIRY` + `TemplateContext::for_storm`.
- `backend/src/routes/settings.rs` — recarga en caliente al guardar claves del breaker.
- `backend/src/routes/export_import.rs` — recarga tras importar settings.
- `frontend/src/pages/Settings.tsx` — pestaña "Tormenta".
- `docs/latency-expiry-aggregation.md` — agrupación de señales (#72).

## Decisiones

- **Sin reintentos/debounce**: la atribución (Fase 3) es el mecanismo correcto; el breaker
  solo agrupa volumen.
- **Umbral absoluto mínimo** (`min_monitors`): evita agregar una sola caída.
- **Dedupe por episodio**: una alerta por tormenta, no una por monitor.
- **Unión de notificadores**: la agregada llega a todos los afectados, no solo al que abre.
- **Estado no persistido**: el breaker arranca limpio; una tormenta a caballo de un reinicio
  se re-evalúa.
- **Un episodio por tipo** (caídas, `latency`, `expiry`): comparten ventana, umbral y
  dedupe, pero no se abren ni se suprimen entre sí (#72).
- **Calibración**: con checks repartidos por jitter, `storm_window_seconds ≳ interval_seconds`
  para que la ráfaga quepa en una sola ventana (M3).

## Follow-ups

- Exponer el estado del breaker (`closed`/`open`) en la API/dashboard para observabilidad,
  incluyendo los episodios de `latency`/`expiry`.
- Contar monitores distintos **sin rearme tras recuperación** (hoy el dedupe vive en la
  ventana; un monitor recuperado conserva su entrada hasta que expira).
- Valorar ventanas o plantillas agregadas diferenciadas por tipo si la operación lo pide.
