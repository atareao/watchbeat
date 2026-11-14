# Design

## Context

Ver `proposal.md` — Why. Tras la Fase 2:
- `FailureCause` clasifica el motivo; `status_for(cause)` da el status base
  (`down`/`error`).
- Pero ese status base no distingue si el fallo es del **target** o de **nuestra
  conectividad de salida**. Los 4 incidentes de producción fueron caídas del DNS
  de salida del host.
- El scheduler (`scheduler.rs`) decide notificación a partir de `is_up`
  (`status == up || warning`): cualquier `down`/`error` alerta.

## Goals / Non-Goals

**Goals**
- Medir la salud de la propia salida sin depender del DNS del sistema.
- Decidir de forma **determinista** a quién se atribuye el fallo.
- Introducir `indeterminate` como estado **no alertable** y distinguible.
- Reflejarlo en notificación, uptime y UI.

**Non-Goals**
- Reintentos (descartados).
- Cambiar las causas (`FailureCause`) ni `status_for` (base, Fase 2).
- Geografía/multi-región; un solo canario local.
- Reconstruir el pasado: sin backfill de checks antiguos.

## Decisions

- **D1 · Canario como task tokio independiente.** `netcheck` corre en su propio
  task con `tokio::time::interval` (default 30 s, `MissedTickBehavior::Skip`),
  separado del scheduler de monitores. No comparte estado mutable con ellos salvo
  el handle de publicación.
- **D2 · Sondeos sin DNS.** Los sondeos de conectividad son **TCP connect por IP**
  a `1.1.1.1:53`, `8.8.8.8:53`, `9.9.9.9:53` (nunca por nombre), para no depender
  del DNS del sistema. Añade **una** resolución DNS de un nombre conocido
  (`cloudflare.com`) vía el resolver propio de la Fase 1 para medir el camino DNS.
- **D3 · Derivación de `NetworkHealth`.**
  - Todos los TCP fallan → `down`.
  - Algún TCP OK pero la resolución falla → `dns_degraded`.
  - Todo OK → `healthy`.
  - Antes del primer sondeo → `unknown` (estado inicial; el clasificador lo trata
    como «sin información» y **no** atribuye a la red).
- **D4 · Publicación vía handle compartido.** `NetworkHealthHandle` (clonable,
  `Send+Sync`) con `current() -> NetworkHealth`. Internamente `tokio::sync::watch`
  (lectura no bloqueante del último valor) o `Arc<AtomicU8>`. El `current()` es
  **no bloqueante** y nunca hace await.
- **D5 · Clasificador puro.** `attribution::classify(cause: Option<FailureCause>,
  health: NetworkHealth) -> &'static str` sin I/O, tabla de verdad explícita:

  | cause | health | status |
  |---|---|---|
  | `None` (OK) | cualquiera | `up` |
  | `DnsNxdomain` | cualquiera | `down` (config del target) |
  | fallo | `Unknown` | status base (`status_for`) |
  | fallo | `Healthy` | status base |
  | `DnsTimeout`/`DnsServfail` | `DnsDegraded` | `indeterminate` |
  | otro fallo | `DnsDegraded` | status base |
  | fallo ≠ `DnsNxdomain` | `Down` | `indeterminate` |

- **D6 · Aplicación en dos puntos.** La atribución se aplica en (a) el path
  programado (`run_monitor_check`) y (b) el `run-check` manual de la ruta, usando
  la salud actual. Así la UI nunca muestra un `down` que el scheduler considerará
  `indeterminate`.
- **D7 · `indeterminate` no es caída.** En el scheduler, `indeterminate` es
  **neutral**: no dispara notificación de caída, no cambia el estado de
  notificación del monitor (`was_up`) y no cuenta como recuperación. Es
  observabilidad, no incidente.
- **D8 · Uptime excluye `indeterminate`.** El denominador del uptime cuenta solo
  filas con status en `{up, warning, down, error}`; `indeterminate` se excluye del
  numerador **y** del denominador (no penaliza al target).

## Risks / Trade-offs

- [El canario genera tráfico saliente periódico] → 3 TCP cortos + 1 resolución /
  30 s; despreciable.
- [Ventana de carrera: el check y el canario no son atómicos] → se usa la última
  salud publicada; el intervalo del canario (30 s) ≫ la duración de un check.
- [`Down` de red oculta un `down` real del target] → es el comportamiento
  **deseado**: durante una caída de red del host no se puede atribuir; se marca
  `indeterminate` y no alerta.
- [`connect_refused`/`http_status` con red `down` podrían ser evidencia de que el
  target sí se alcanzó] → se ignora en esta fase (verdad simplificada del plan);
  anotado como follow-up.

## Migration Plan

- Sin migración de datos ni de esquema. `indeterminate` es un valor nuevo de
  `checks.status`.
- Despliegue normal. Rollback = revertir el PR (los checks `indeterminate`
  existentes quedarían como un valor no reconocido; inocuo).

## Open Questions

- Intervalo del canario: default 30 s fijo o configurable por env/settings. No
  cambia las specs; se decide en implementación (default 30 s).
