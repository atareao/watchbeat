# PLAN-002 — Robust Check Engine (Fases 1-6)

- **Issue paraguas**: [#34](https://github.com/atareao/watchbeat/issues/34)
- **Estado**: Planificado (ejecución fase a fase)
- **Fecha**: 13 Nov 2026
- **Rama base**: `development`
- **Método**: OpenSpec (SDD) + TDD (Red-Green-Refactor), un change proposal por fase
- **Política**: TODO se traza por issue de GitHub; cada fase tiene su sub-issue y su documentación

---

## 1. Contexto y problema

WatchBeat en Co1 reporta caídas **simultáneas de todos los monitores** que no corresponden a caídas reales de los servicios. Evidencia de la DB de producción (`checks`, 156 770 filas):

- 4 incidentes correlacionados en 2 semanas (02-nov ×2, 04-nov, 13-nov).
- `response_time_ms ≈ 5000` en todos, **no** los 30 000 del `timeout_seconds` configurado → el fallo es de **resolución de nombres (DNS)**, antes de abrir conexión HTTP.
- Fallan solo los dominios que requieren DNS externo; los que resuelven al propio host sobreviven.

**Causa raíz**: microcaídas del DNS/red de salida del host.
**Problema de fondo a resolver**: WatchBeat no distingue "falló mi red" de "falló el servicio". En `backend/src/checker/mod.rs` cualquier error de red se guarda como `error`/`down`, indistinguible de una caída real.

---

## 2. Objetivo

Blindar WatchBeat para que **la caída de la infraestructura nunca se confunda con la caída de los servicios**, sin depender de la fiabilidad de la red del host. La detección debe seguir siendo **inmediata** (una sola pasada por check); lo que cambia es que la decisión es correcta.

## 3. No-objetivos

- **Reintentos**: descartados por diseño. No aportan información nueva, enmascaran flapping y añaden latencia.
- **`confirmations_required` / debounce temporal**: no se usará como mecanismo de atribución.
- **Multi-vantage** (segundo punto de observación / dead-man's switch): fuera de este alcance.

---

## 4. Arquitectura — los 6 pilares

### Pilar 1 · Resolución propia con caché
Resolver embebido (`hickory-resolver`) con upstreams múltiples + fallback + caché con TTL respetado y refresco proactivo. El DNS del host deja de estar en el camino crítico del check.

### Pilar 2 · Clasificación de la causa
`CheckOutcome` incorpora una `FailureCause` determinista. Se persiste `error_kind` en `checks` para diagnóstico.

Variantes de `FailureCause`:
`DnsNxdomain`, `DnsTimeout`, `DnsServfail`, `ConnectRefused`, `ConnectTimeout`, `NetworkUnreachable`, `TlsError`, `HttpStatus`, `BodyMismatch`, `Other`.

### Pilar 3 · Canario / NetworkHealth
Task en background que publica la salud de la propia salida **sin depender del DNS**: TCP a `1.1.1.1:53`, `8.8.8.8:50`, `9.9.9.9:53` por IP + consulta DNS de un nombre conocido vía el resolver propio. Estados: `healthy` / `dns_degraded` / `down`. Estado publicado vía canal/atómico compartido y leído por el clasificador.

### Pilar 4 · Atribución y estado `indeterminate`
Regla determinista (causa + salud de red en el instante del check):

| Target | Red | Resultado |
|---|---|---|
| falla | healthy | `down` (caída real → alerta) |
| falla | dns_degraded + causa `dns_*` | `indeterminate` |
| falla | down | `indeterminate` |
| `dns_nxdomain` | — | `down` (config del target) |

Nuevo estado `indeterminate`, visible en UI y distinguible de `down`.

### Pilar 5 · Jitter
Desincronizar el primer tick e intervalo de cada monitor para evitar el thundering herd (hoy los 8 monitores cronan en el mismo segundo).

### Pilar 6 · Anti-tormenta / breaker
Si el canario está caído o fallan más de X % de monitores en menos de Y segundos → una sola alerta agregada "monitorización degradada" en lugar de N alertas individuales.

---

## 5. Contratos técnicos

### Backend (Rust)

- `backend/src/checker/mod.rs`
  - `pub enum FailureCause { Nxdomain, DnsTimeout, DnsServfail, ConnectRefused, ConnectTimeout, NetworkUnreachable, TlsError, HttpStatus, BodyMismatch, Other }`
  - `CheckOutcome` añade `pub cause: Option<FailureCause>`.
  - El `HttpChecker`, `TcpChecker`, `PingChecker` y `TlsChecker` clasifican la causa desde el error real (no repiten la petición).
- `backend/src/dns.rs` (nuevo): resolver con caché (`hickory-resolver`), API `resolve(host) -> Result<IpAddr, DnsError>` con `DnsError { kind } `.
- `backend/src/netcheck.rs` (nuevo): canario. `pub enum NetworkHealth { Healthy, DnsDegraded, Down }`; task `spawn_netcheck(interval) -> watch::Receiver<NetworkHealth>`; config por env/settings.
- `backend/src/classify.rs` (nuevo): función pura `classify(outcome_cause, network_health) -> Status`.
- `backend/src/scheduler.rs`: `run_monitor_check` consume `NetworkHealth` y aplica `classify`; añade jitter al primer tick.
- `backend/src/db.rs`: migración `ALTER TABLE checks ADD COLUMN error_kind TEXT`; persistir `error_kind `.
- `backend/src/models.rs`: `CheckResult`/`CheckOutcome` con causa; estado `indeterminate` contemplado en toda la lógica de `is_up`.

### Estados y semántica

- Valores válidos de `checks.status`: `up`, `warning`, `down`, `error`, `indeterminate`.
- `indeterminate` **no** cuenta como caída para notificaciones.
- La lógica `is_up` (`scheduler.rs`, `db.rs`, `routes/status.rs`) debe tratar `indeterminate` explícitamente.

### Frontend (React + TypeScript)

- Tipos de estado en `frontend/src/**` que incluyan `indeterminate`.
- `MonitorCard`, `Dashboard`, `MonitorDetail` y timeline: badge/color propio para `indeterminate`.
- Sin test runner configurado en frontend → verificación con `npx tsc --noEmit` y `npm run build` + prueba manual.

### Configuración / settings

- Intervalo del canario (default 10 s), endpoints de sonda y umbrales anti-tormenta (X %, Y s) configurables.

---

## 6. Trazabilidad — Issues en GitHub

**Todo se traza por issue.** El flujo por fase es:

1. **Sub-issue existente** (creado y colgado del paraguas #34) descrito en la tabla de §7.
2. Rama `feature/<slug>` desde `development`.
3. Change proposal de OpenSpec aprobado.
4. TDD (Red-Green-Refactor) con verificación por CLI.
5. PR a `development` referenciando el sub-issue (`Closes #NN`).
6. `docs/<feature>.md` + `openspec archive`.
7. **Cerrar el sub-issue** solo con tests verdes + doc publicada.

Issue paraguas [#34](https://github.com/atareao/watchbeat/issues/34): se cierra en la Fase 6, cuando todos los sub-issues estén cerrados.

| Fase | Sub-issue | Change OpenSpec | Rama |
|---|---|---|---|
| 1 | [#35](https://github.com/atareao/watchbeat/issues/35) | `robust-dns-resolver` | `feature/robust-dns-resolver` |
| 2 | [#36](https://github.com/atareao/watchbeat/issues/36) | `failure-cause-taxonomy` | `feature/failure-cause-taxonomy` |
| 3 | [#37](https://github.com/atareao/watchbeat/issues/37) | `network-attribution` | `feature/network-attribution` |
| 4 | [#38](https://github.com/atareao/watchbeat/issues/38) | `scheduler-jitter` | `feature/scheduler-jitter` |
| 5 | [#39](https://github.com/atareao/watchbeat/issues/39) | `alert-storm-breaker` | `feature/alert-storm-breaker` |
| 6 | [#40](https://github.com/atareao/watchbeat/issues/40) | `robust-check-engine-docs` | `feature/robust-check-engine-docs` |

---

## 7. Fases y tareas

> Cada fase = 1 sub-issue + 1 change proposal de OpenSpec + ciclo TDD completo + PR + doc + `openspec archive`. No se empieza una fase sin cerrar la anterior.

### Fase 1 · Resolución propia con caché — [#35](https://github.com/atareao/watchbeat/issues/35)
- [x] `openspec new change robust-dns-resolver`
- [x] Spec: contratos de `dns.rs`, escenarios (resolución OK, TTL/caché, caída de upstream con fallback, NXDOMAIN, timeout).
- [x] RED: tests de resolución y caché + fallback.
- [x] GREEN: implementar `backend/src/dns.rs` con `hickory-resolver`; integrarlo en checkers (`http`, `tcp`, `tls`) para resolver por IP.
- [x] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt`.
- [x] Docs: `docs/robust-dns-resolver.md`.
- [x] PR a `development` (`Closes #35`) + archive.

### Fase 2 · Taxonomía de causas + `error_kind` — [#36](https://github.com/atareao/watchbeat/issues/36)
- [ ] `openspec new change failure-cause-taxonomy`
- [ ] Spec: enum `FailureCause`, mapa error→causa, columna `error_kind `.
- [ ] RED: tests de clasificación por tipo de error (HTTP/TCP/TLS).
- [ ] GREEN: añadir `cause` a `CheckOutcome`, clasificar en cada checker, migración + persistencia de `error_kind `.
- [ ] REFACTOR: limpieza de la heurística `is_timeout()`.
- [ ] Docs: `docs/failure-cause-taxonomy.md`.
- [ ] PR a `development` (`Closes #36`) + archive.

### Fase 3 · Canario, clasificador e `indeterminate` — [#37](https://github.com/atareao/watchbeat/issues/37)
- [ ] `openspec new change network-attribution`
- [ ] Spec: `NetworkHealth`, task canario, `classify()`, semántica de `indeterminate`, efectos en notificación.
- [ ] RED: tests de `classify()` (tabla de verdad completa) y del canario (probes por IP, sin DNS).
- [ ] GREEN: `netcheck.rs`, `classify.rs`, integrar en `run_monitor_check`, estado `indeterminate` en DB/API, frontend (badges + timeline + tipos).
- [ ] REFACTOR: clippy/tsc/build.
- [ ] Docs: `docs/network-attribution.md`.
- [ ] PR a `development` (`Closes #37`) + archive.

### Fase 4 · Jitter — [#38](https://github.com/atareao/watchbeat/issues/38)
- [ ] `openspec new change scheduler-jitter`
- [ ] Spec: distribución del jitter (±10 % del intervalo) y desincronización del primer tick.
- [ ] RED: test estadístico de que los ticks no colisionan / rango correcto (RNG inyectable).
- [ ] GREEN: aplicar jitter en `monitor_task`/`SchedulerManager`.
- [ ] REFACTOR: clippy/fmt.
- [ ] Docs: `docs/scheduler-jitter.md`.
- [ ] PR a `development` (`Closes #38`) + archive.

### Fase 5 · Breaker / anti-tormenta — [#39](https://github.com/atareao/watchbeat/issues/39)
- [ ] `openspec new change alert-storm-breaker`
- [ ] Spec: detección de tormenta (X %, Y s), alerta agregada, dedupe/ventana, estados.
- [ ] RED: tests del breaker (muchos monitores fallan a la vez → 1 alerta).
- [ ] GREEN: lógica de agregación en el despacho de notificadores + settings + UI de config.
- [ ] REFACTOR: clippy/tsc/build.
- [ ] Docs: `docs/alert-storm-breaker.md`.
- [ ] PR a `development` (`Closes #39`) + archive.

### Fase 6 · Documentación integral y cierre — [#40](https://github.com/atareao/watchbeat/issues/40)
- [ ] `openspec new change robust-check-engine-docs`
- [ ] `docs/robust-check-engine.md` — visión global: **qué** se hizo, **por qué** (razones y decisiones), **cómo** funciona.
- [ ] Consolidar y enlazar los `docs/<feature>.md` de las fases 1-5.
- [ ] ADR de las decisiones clave (descartar reintentos en favor de atribución; `indeterminate` como estado de primera clase; DNS propio vs DNS del host).
- [ ] Actualizar `README.md`, `CHANGELOG.md`, `watchbeat.env.example` (nuevas env vars), `GIT_FLOW.md` y la guía de self-hoster.
- [ ] **Informe final**: resumen ejecutivo + qué se ha realizado + razones + métricas antes/después (falsos positivos, nº de alertas, latencias).
- [ ] Cerrar sub-issues #35-#39 (si no lo estuvieran) y este #40; cerrar el paraguas #34.
- [ ] Renombrar `plans/PLAN-002.md` → `plans/PLAN-002-ARCHIVED.md`.
- [ ] PR a `development` (`Closes #34, #40`).

---

## 8. Criterios de aceptación (global)

- [ ] Un blip de DNS/red del host **no** produce notificaciones de caída de servicios.
- [ ] La causa de cada check fallido queda clasificada y persistida (`error_kind`).
- [ ] El estado `indeterminate` es visible en UI y distinguible de `down`.
- [ ] Los checks de distintos monitores no se disparan en el mismo segundo.
- [ ] Una caída de infraestructura genera como máximo una alerta agregada.
- [ ] Toda la documentación (global + por fase + ADR + informe final) está en `docs/`.
- [ ] Todos los sub-issues (#35-#40) cerrados y `plans/PLAN-002.md` renombrado a `-ARCHIVED`.
- [ ] `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` y `npx tsc --noEmit` en verde al cierre de cada fase.

---

## 9. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| `hickory-resolver` añade peso/dependencia | Confinado en `dns.rs`, API interna estrecha |
| El canario también depende de la red (no puede sondear si todo está caído) | Sondas por IP sin DNS + múltiples upstreams; en duda → `indeterminate`, nunca falso `down` |
| `indeterminate` roza la semántica de "down silencioso" | Persistir `error_kind` y exponerlo; el estado comunica "no atribuible", no "oculto" |
| Cambios de estado rompen UI/status pages | Fase 3 incluye todos los puntos de render y la API pública `/status/{slug}` |
| Desincronización del jitter rompe tests deterministas | Inyectar RNG/semilla en tests |

---

## 10. Proceso

1. Un sub-issue por fase (ya creados, #35-#40), colgados del paraguas #34.
2. Un change proposal OpenSpec por fase, aprobado antes de escribir código.
3. TDD estricto: RED → GREEN → REFACTOR, con verificación por CLI.
4. PR a `development` por fase, con `Closes #NN`.
5. Al cerrar cada fase: `docs/<feature>.md` + `openspec archive` + actualizar este plan.
6. La Fase 6 consolida la documentación global, el ADR y el informe final, cierra todos los issues y archiva este plan.

---

## 11. Referencias

- `backend/src/checker/mod.rs` (clasificación de errores, cliente HTTP)
- `backend/src/scheduler.rs` (`run_monitor_check`, bucle de monitors)
- `backend/src/db.rs` (esquema `checks`, migraciones)
- `HANDOFF.md` (diagnóstico previo de falsos positivos)
- Issue paraguas #34 + sub-issues #35-#40
