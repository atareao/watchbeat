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

- **Estados**: `Closed` (normal) y `Open` (tormenta en curso).
- **Detección**: se declara tormenta cuando el número de **monitores únicos** que pasan a
  `down` dentro de una ventana de `Y` segundos alcanza
  `max(min_monitors, ceil(X % · monitores_habilitados))`.
  - Si `monitores_habilitados < min_monitors`, **nunca** se declara tormenta (despliegues
    pequeños conservan la semántica clásica de una alerta por monitor).
  - El recuento es por **monitor único** (dedupe por `monitor_id`): un monitor que flapea
    no dispara una falsa tormenta.
- **Al abrir**: se emite **una** alerta agregada con el recuento y la lista de monitores
  afectados, a la **unión deduplicada** de los notificadores de todos los afectados.
- **Dedupe**: durante `Open` no se repite la agregada aunque sigan cayendo monitores.
- **Supresión**: durante `Open` se suprimen las transiciones individuales `down` **y** `up`.
- **Rearme**: al pasar la ventana `Y` sin nuevas caídas, vuelve a `Closed`; un nuevo
  episodio puede volver a alertar.

## Configuración

Claves en la tabla `settings` (KV), editables desde la pestaña **"Tormenta"** de
`Settings`:

| Clave | Default | Descripción |
|---|---|---|
| `storm_breaker_enabled` | `true` | Activa/desactiva el breaker. `false` = comportamiento clásico. |
| `storm_threshold_percent` | `50` | `X`: % de monitores habilitados que deben caer. |
| `storm_window_seconds` | `10` | `Y`: ventana de detección en segundos. |
| `storm_min_monitors` | `3` | Suelo absoluto de monitores para declarar tormenta. |
| `default_template_storm` | (vacío) | Plantilla de la alerta agregada. |

La configuración se aplica **en caliente**: guardar en la UI (o importar settings) emite
`SchedulerCommand::ReloadBreaker`, que reconfigura el breaker sin reiniciar.

### Plantilla de la alerta agregada

Contexto disponible en `default_template_storm`:

- `{{ count }}` — número de monitores afectados.
- `{{ monitors_list }}` — nombres separados por comas.
- `{{ monitors }}` — array de nombres.
- `{{ window_seconds }}` — ventana `Y`.
- `{{ threshold_percent }}` — umbral `X`.

Si no se configura, se usa la plantilla por defecto `template::defaults::STORM`.

## Ficheros implicados

- `backend/src/breaker.rs` — máquina de estados, config, decisión, tests.
- `backend/src/scheduler.rs` — `Arc<Mutex<StormBreaker>>` compartido, punto de decisión en
  el despacho, `ReloadBreaker`, unión de notificadores.
- `backend/src/template.rs` — `defaults::STORM` + `TemplateContext::for_storm`.
- `backend/src/routes/settings.rs` — recarga en caliente al guardar claves del breaker.
- `backend/src/routes/export_import.rs` — recarga tras importar settings.
- `frontend/src/pages/Settings.tsx` — pestaña "Tormenta".

## Decisiones

- **Sin reintentos/debounce**: la atribución (Fase 3) es el mecanismo correcto; el breaker
  solo agrupa volumen.
- **Umbral absoluto mínimo** (`min_monitors`): evita agregar una sola caída.
- **Dedupe por episodio**: una alerta por tormenta, no una por monitor.
- **Unión de notificadores**: la agregada llega a todos los afectados, no solo al que abre.
- **Estado no persistido**: el breaker arranca limpio; una tormenta a caballo de un reinicio
  se re-evalúa.

## Follow-ups

- Exponer el estado del breaker (`closed`/`open`) en la API/dashboard para observabilidad.
- Valorar agrupar también `latency`/`expiry` (hoy solo `down`/`up` participan).
