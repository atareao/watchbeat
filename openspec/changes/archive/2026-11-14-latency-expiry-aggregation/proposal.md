# Proposal

**Issue:** [#72](https://github.com/atareao/watchbeat/issues/72)
**Plan:** `plans/PLAN-015.md`

## Why

El storm breaker (`backend/src/breaker.rs`) ya agrupa y suprime las notificaciones de caída
(`down`/`up`) cuando varios monitores caen dentro de una ventana. Sin embargo, los avisos de
`latency` y `expiry` (breach del umbral de latencia y expiración de certificado TLS) se
despachan **siempre** de forma individual: en `scheduler.rs` el breaker sólo interviene para
`down`/`up` y cualquier otro tipo de notificación recibe `BreakerDecision::Pass`. Una
degradación de red o de un proveedor dispara así ráfagas de avisos de latencia (uno por
monitor y por check por encima del umbral) o de expiración que saturan los canales de
notificación — justo el escenario que el breaker existe para evitar. Es el bloque `e` de #62.

## What Changes

- El breaker gana una máquina de agrupación **por tipo de señal** para `latency` y `expiry`,
  análoga a la de caídas: misma ventana `Y` y mismo umbral
  `max(min_monitors, ceil(X % · monitores_habilitados))`, contabilizando **monitores
  distintos** que emiten la misma señal dentro de la ventana.
- Nuevo punto de entrada
  `StormBreaker::on_signal(now, kind, monitor_id, monitor_name, enabled_monitors) -> BreakerDecision`,
  con `SignalKind { Latency, Expiry }`.
- Cada tipo de señal mantiene su **propio episodio independiente** (una tormenta de latencia
  no suprime las de expiración, ni al revés, ni afecta a la de caídas), con rearme perezoso
  tras `Y` segundos sin nuevas señales de ese tipo.
- Al alcanzar el umbral se emite **una única alerta agregada** de ese tipo (plantillas nuevas
  `defaults::STORM_LATENCY` / `defaults::STORM_EXPIRY`, listando recuento y monitores) y las
  señales siguientes del mismo tipo se suprimen hasta el rearme.
- `scheduler.rs` enruta las notificaciones `latency`/`expiry` por `on_signal`; `Pass` →
  individual, `Suppress` → descartada, `OpenAggregate` → mensaje agregado (a la unión de
  notificadores de los monitores afectados, igual que en la tormenta de caídas).
- Con `storm_breaker_enabled = false` el comportamiento vuelve a ser el clásico (todas las
  señales se notifican individualmente).

**Alcance:** sólo backend. No hay cambio de UI (el badge del dashboard describe la tormenta de
caídas; la agrupación de latency/expiry es invisible salvo por menos avisos). Exponer las
señales en `GET /api/breaker` queda como follow-up.

## Capabilities

### Added Capabilities

- `breaker`: el breaker SHALL agrupar las notificaciones de latencia y expiración por ventana
  y umbral, emitiendo una única alerta agregada por episodio y suprimiendo las individuales.

## Impact

- `backend/src/breaker.rs` (`SignalKind`, `on_signal`, episodios por tipo, tests unitarios).
- `backend/src/scheduler.rs` (enrutado de `latency`/`expiry` por `on_signal`; render del agregado).
- `backend/src/template.rs` (`defaults::STORM_LATENCY`, `defaults::STORM_EXPIRY`).
- `docs/latency-expiry-aggregation.md`.
- Sin migración de esquema: la configuración reutiliza los settings existentes; el estado no se persiste.
