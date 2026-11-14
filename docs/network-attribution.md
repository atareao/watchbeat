# Network attribution y estado `indeterminate` (Fase 3 · Robust Check Engine)

- **Issue**: [#37](https://github.com/atareao/watchbeat/issues/37)
- **Plan**: `plans/PLAN-002-ARCHIVED.md` — Robust Check Engine (Fase 3 de 6)
- **Change OpenSpec**: `openspec/changes/archive/…-network-attribution/`

## Qué es

La pieza que decide **a quién pertenece un fallo de check**: al **target** o a
**nuestra propia conectividad de salida**. Combina un canario de salud de red
(`NetworkHealth`) con la causa del fallo (`FailureCause`, Fase 2) para producir un
estado **atribuido**, incluido el nuevo **`indeterminate`** («no atribuible»).

## Por qué

Aunque la Fase 1 cachea el DNS y la Fase 2 etiqueta la causa, un fallo por
infraestructura del host (caída de red, DNS de salida degradado) seguía
registrándose como `down`/`error` y **alertaba como si el servicio estuviera
caído**. `indeterminate` es observabilidad, no incidente: no alerta, no penaliza
el uptime y se muestra distinto de `down`.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/netcheck.rs` | **Nuevo.** Canario `NetworkHealth` + handle compartido |
| `backend/src/attribution.rs` | **Nuevo.** `classify` (tabla pura) y `attribute` (integración) |
| `backend/src/scheduler.rs` | Aplica la atribución; `ObservedStatus`; `indeterminate` neutral |
| `backend/src/db.rs` | Uptime, timeline y dashboard excluyen/etiquetan `indeterminate` |
| `backend/src/models.rs` | `DashboardStatus.indeterminate_monitors` |
| `backend/src/routes/status_pages.rs` | Página pública distingue `indeterminate` |
| `backend/src/routes/monitors.rs` | `run-check` manual usa la misma atribución |
| `backend/src/main.rs` / `auth.rs` | Arranque del canario + `AppState.netcheck` |
| `frontend/src/{components,pages,hooks,api}` | Badge violeta, filtros, stat card y `error_kind` |

## Cómo funciona

### 1. Canario de salud (`netcheck`)

Task tokio propio (intervalo **30 s**, `MissedTickBehavior::Skip`; timeout de
sondeo **5 s**). Sondeos:

- **TCP por IP literal** a `1.1.1.1:53`, `8.8.8.8:53`, `9.9.9.9:53` (nunca por
  nombre → no dependen del DNS del sistema).
- **Una** resolución DNS de un nombre conocido (`cloudflare.com`) vía el resolver
  propio de la Fase 1.

Derivación de estado:

| Situación | `NetworkHealth` |
|---|---|
| Todos los TCP fallan | `down` |
| Algún TCP OK pero la resolución falla | `dns_degraded` |
| Todo OK | `healthy` |
| Antes del primer ciclo | `unknown` |

Un sondeo que expira cuenta como fallo. Un panic en un ciclo **no** detiene el
canario (se recupera y conserva la salud previa). La salud se publica en un
`NetworkHealthHandle` clonable con lectura **no bloqueante**.

### 2. Atribución (`attribution`)

`classify(cause, health)` es la tabla pura; `attribute(base_status, cause, health)`
es la integración usada por el scheduler:

- `up`/`warning` → sin cambios.
- `DnsNxdomain` → `down` (config del target, siempre).
- fallo con `cause = None` o `Some(...)` y `health = down` → `indeterminate`.
- `dns_timeout`/`dns_servfail` + `health = dns_degraded` → `indeterminate`.
- resto → status base (`status_for(cause)`).

Que un fallo **sin causa** (p. ej. `ping`) también se atribuya es lo que evita
que un monitor `ping` siga alertando `down` durante un corte de red del host.

### 3. Semántica de `indeterminate` (D7)

- **No** dispara notificación de caída, **no** cambia `was_up` (estado de
  notificación) y **no** cuenta como recuperación.
- **No** toca `failed_checks`/`confirmations_required`.
- La `error_kind` (causa) se sigue persistiendo.
- Un corte sostenido emite **un** evento SSE (en la transición), no uno por check.
- Uptime: se excluye del numerador **y** del denominador.
- Timeline: un bucket sin checks atribuibles emite `dominant_status="indeterminate"`.

## Variables / configuración

Ninguna nueva. El intervalo (30 s) y el timeout (5 s) del canario son constantes
internas. Sin migración de DB (`checks.status` es texto libre).

## Cómo habilitar / desplegar

- **Activo por defecto**. Sin cambios de API (aditivos) ni migración.
- Rollback = revertir el PR. Los checks `indeterminate` existentes quedarían como
  un valor no reconocido (inocuo).

## Decisiones

- **`attribute` además de `classify`**: `classify(None, _) = up` habría convertido
  en `up` el `warning` de TLS y los fallos sin causa. `attribute` solo eleva un
  fallo a `indeterminate`; todo lo demás conserva el status del checker.
- **`ObservedStatus`** en el scheduler para comparar contra el último estado
  **persistido** (evita amplificar registro/SSE durante un corte sostenido).
- **Verdad simplificada**: durante una caída de red del host, un `down` real del
  target se marca `indeterminate` (no se puede atribuir) → es el comportamiento
  **deseado**.
- **Frontend**: violeta `#722ed1` + `QuestionCircleOutlined` (doble canal, color +
  forma) para no depender solo del color.

## Tests

Backend: 204 unit + 45 db_integration + 3 run-check + 9 api_keys, deterministas y
**sin red externa** (sondeos y upstream DNS inyectados, webhook en loopback).
Cubren la tabla de verdad (incl. delegación `attribute`→`classify`), el canario
(estados, timeout, panic recovery, intervalo), las transiciones
(`up→indeterminate→up`, `down→indeterminate→down`, primer check), la neutralidad
de notificación, uptime/timeline/dashboard y la status page. Frontend: `tsc
--noEmit` + `npm run build`.

## Follow-ups

- **a11y del timeline** (barras distinguibles solo por color; tooltip hover-only):
  preexistente.
- **`connect_refused`/`http_status` con red `down`** se marcan `indeterminate`
  aunque el target pudo ser alcanzado: verdad simplificada del plan.
- **Intervalo del canario configurable** por env/settings (hoy fijo a 30 s).
- La `error_kind` no viaja en el evento SSE `check` (se rellena con el refetch).
