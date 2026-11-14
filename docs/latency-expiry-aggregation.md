# Agrupación de señales de latencia y expiración (#72)

## Qué es

El anti-tormenta (`docs/alert-storm-breaker.md`) nació agrupando **caídas**: cuando varios
monitores pasan a `down` dentro de una ventana se emite **una** alerta agregada en lugar de
N individuales. Sin embargo, `latency` (breach del umbral de latencia) y `expiry`
(certificado TLS próximo a expirar) se despachaban **siempre** una por monitor y por check.

Una degradación de red o de un proveedor compartido dispara ráfagas de latencia (un aviso
por monitor y por check por encima del umbral) o de expiración (renovaciones TLS olvidadas
en el mismo proveedor). Esa ráfaga satura los canales exactamente igual que una tormenta de
caídas, así que desde #72 pasa por la **misma** lógica de agrupación:

- **una ráfaga = una alerta agregada** del tipo correspondiente;
- el resto de señales del episodio se **suprimen** hasta el rearme.

No hay cambios de esquema ni settings nuevos.

## Ficheros implicados

| Fichero | Responsabilidad |
|---|---|
| `backend/src/breaker.rs` | `SignalKind { Latency, Expiry }`, `on_signal`, episodios independientes (`Episode`) por tipo, `WhileOpen` (política de refresco), tests |
| `backend/src/scheduler.rs` | Enrutado de las notificaciones `latency`/`expiry` por `on_signal`; render del agregado; unión de notificadores |
| `backend/src/template.rs` | `defaults::STORM_LATENCY`, `defaults::STORM_EXPIRY` y sus tests |

## Cómo funciona

`StormBreaker` mantiene **tres episodios independientes**:

| Episodio | Señales que lo alimentan | Alerta agregada |
|---|---|---|
| `down` | transiciones `up → down` (y `up` para el rearme) | `default_template_storm` (settings) → `defaults::STORM` |
| `latency` | breach del umbral de latencia | `defaults::STORM_LATENCY` |
| `expiry` | `cert_days_left < config_json.expiry_days` | `defaults::STORM_EXPIRY` |

Los tres comparten **la misma ventana** `Y` (`storm_window_seconds`) y **el mismo umbral**
`max(min_monitors, ceil(X % · monitores_habilitados))`, y cuentan **monitores distintos**
(dedupe por `monitor_id`: un monitor que repite la señal refresca su entrada, no suma otra).

Punto de entrada nuevo:

```rust
StormBreaker::on_signal(now, kind, monitor_id, monitor_name, enabled_monitors)
    -> BreakerDecision
```

Decisiones posibles y qué hace el scheduler con cada una:

| `BreakerDecision` | Scheduler |
|---|---|
| `Pass` | Notifica **individualmente**, como siempre. |
| `Suppress` | **Descarta** la notificación (el agregado del episodio ya salió). |
| `OpenAggregate(alert)` | Construye el mensaje con la plantilla del tipo y lo envía a la **unión** de notificadores de los monitores afectados. |

Detalles del episodio:

- **Independencia**: abrir o suprimir `latency` no toca `expiry` ni las caídas, y viceversa.
- **Rearme perezoso** por tipo: transcurrida la ventana `Y` sin **nuevas señales
  registradas** de ese tipo, el episodio vuelve a `Closed` y se limpia; la siguiente ráfaga
  vuelve a alertar.
- **No se refresca mientras está `Open`** (M2): un monitor con
  `interval_seconds < storm_window_seconds` re-emite la señal en cada check. Si esas señales
  suprimidas refrescasen su evento, el episodio no rearmaría **nunca** y todas las señales
  del tipo quedarían silenciadas para siempre. Por eso, con el episodio abierto, la señal se
  descarta sin registrar. (El episodio de caídas conserva el refresco documentado en
  MAYOR-1: un `down` nuevo indica que la flota sigue cayendo.)
- **Suelo absoluto**: con menos monitores habilitados que `min_monitors` nunca se declara
  tormenta de señal (`Pass` siempre).
- **Reloj inyectado**: `now: Instant` en cada llamada; los tests avanzan el reloj a mano,
  sin `sleep` reales.
- **Lock**: el scheduler toma `Arc<Mutex<StormBreaker>>`, decide (trabajo síncrono) y suelta
  el lock **antes** de cualquier `await`. Un agregado supone como mucho una lectura extra de
  settings (solo para el tipo `down`).

## Configuración

Se reutilizan los settings existentes del breaker, sin claves nuevas:

| Clave | Default | Efecto sobre las señales |
|---|---|---|
| `storm_breaker_enabled` | `true` | `false` → `latency`/`expiry` se notifican **siempre** individualmente (comportamiento clásico). |
| `storm_threshold_percent` | `50` | `X`: % de monitores habilitados que deben emitir la señal. |
| `storm_window_seconds` | `10` | `Y`: ventana de agrupación. |
| `storm_min_monitors` | `3` | Suelo absoluto; por debajo, nunca se agrupa. |

`default_template_storm` **no** aplica a estas señales: usan las plantillas por defecto
`STORM_LATENCY`/`STORM_EXPIRY` (no se añadieron settings nuevos). Las plantillas
individuales siguen siendo `default_template_latency` / `default_template_expiry` y sus
overrides por monitor.

### Cómo se habilita / deshabilita

- **Deshabilitar**: `storm_breaker_enabled = false` (pestaña *Tormenta* de `Settings`, o
  cualquier otro camino que guarde settings). Se aplica en caliente: la recarga
  (`SchedulerCommand::ReloadBreaker`) reconfigura el breaker, cierra los episodios y **no**
  invalida la configuración de plantillas.
- **Habilitar**: `storm_breaker_enabled = true` (valor por defecto).

### Nota de calibración (M3)

`enabled_monitors` se muestrea al vuelo y los checks van repartidos por **jitter**, así que
las señales de una misma incidencia se dispersan en el tiempo. Conviene que
`storm_window_seconds` sea **≳ `interval_seconds`** del monitor más lento del grupo: con una
ventana mucho menor que el intervalo, dos monitores afectados pueden caer en ventanas
distintas y el agregado no llegaría a dispararse (se notificarían por separado).

## Decisiones

- **Reutilizar antes que duplicar**: un solo `Episode` (estado + deque de eventos) y una
  sola función `decide` sirven a caídas y señales; lo único que cambia es la política
  `WhileOpen` y el tipo de plantilla.
- **Mismo umbral para todo**: si `latency` tuviese un umbral propio habría dos fuentes de
  configuración y dos formas de explicar "tormenta"; el operador ya calibra `X`/`Y`/suelo.
- **Una plantilla por tipo**: el mensaje agregado dice *qué* señal se agrupó (latencia ≠
  certificado), sin inventar settings nuevos.
- **Contexto compartido** (`TemplateContext::for_storm`): `count`, `monitors`,
  `monitors_list`, `window_seconds`, `threshold_percent` en los tres agregados.
- **El estado de señal no se expone**: `GET /api/breaker` y el badge del dashboard siguen
  describiendo la tormenta de caídas (ver follow-ups).
- **`state()` intacto**: devuelve el estado del episodio de caídas; los episodios de señal
  son internos.

## Follow-ups

- Exponer los episodios `latency`/`expiry` en `GET /api/breaker` y, si aporta, en el
  dashboard (hoy la agrupación es invisible salvo por el menor número de avisos).
- Contar "monitores distintos **sin rearme tras recuperación**": hoy el dedupe vive en la
  ventana; un monitor que se recupera y vuelve a brechar dentro de `Y` sigue ocupando su
  entrada (no infla el recuento, pero tampoco se libera hasta que expira la ventana).
- Plantillas agregadas configurables por tipo (`default_template_storm_latency`,
  `default_template_storm_expiry`) si se pide personalización; hoy son constantes.
- Valorar `storm_window_seconds` distinto por tipo si latencia y expiración necesitan
  ventanas de calibración muy distintas.