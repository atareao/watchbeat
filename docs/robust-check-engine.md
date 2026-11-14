# Robust Check Engine

> Visión global de las fases 1–5 del plan `plans/PLAN-002-ARCHIVED.md`.
> Issue paraguas: [#34](https://github.com/atareao/watchbeat/issues/34).

## 1. El problema

WatchBeat monitoriza servicios desde un host. Cuando **la infraestructura del host**
(DNS de salida, red, un tramo intermedio) sufre un microcorte, **todos** los monitores
fallan a la vez y el sistema los registraba como `down`/`error`, alertando como si los
servicios estuvieran caídos. Evidencia real (host `co1`):

- fallos **simultáneos en el mismo segundo** en monitores no relacionados;
- `response_time_ms ≈ 5000` (timeout de resolución DNS);
- solo afectaba a dominios con DNS externo;
- 8 monitores con `interval_seconds = 60` disparando en el mismo segundo.

El resultado eran **falsos positivos** y **tormentas de alertas** por un único suceso de
infraestructura.

## 2. La solución en 6 fases

| Fase | Qué aporta | Issue | Doc |
|---|---|---|---|
| 1 · DNS propio | Resolver con caché TTL + fallback multi-upstream; un corte del DNS del host no rompe los checks | [#35](https://github.com/atareao/watchbeat/issues/35) | [robust-dns-resolver.md](robust-dns-resolver.md) |
| 2 · Taxonomía de causas | `FailureCause` + `error_kind`: cada fallo se etiqueta *por qué* falló | [#36](https://github.com/atareao/watchbeat/issues/36) | [failure-cause-taxonomy.md](failure-cause-taxonomy.md) |
| 3 · Atribución | Canario `NetworkHealth` + `indeterminate`: el fallo se atribuye al target o a nuestra red | [#37](https://github.com/atareao/watchbeat/issues/37) | [network-attribution.md](network-attribution.md) |
| 4 · Jitter | Desincroniza los ticks de los monitores (anti *thundering herd*) | [#38](https://github.com/atareao/watchbeat/issues/38) | [scheduler-jitter.md](scheduler-jitter.md) |
| 5 · Breaker | Una caída en cascada genera **una** alerta agregada, no N | [#39](https://github.com/atareao/watchbeat/issues/39) | [alert-storm-breaker.md](alert-storm-breaker.md) |
| 6 · Cierre | Documentación global, ADR, informe y cierre | [#40](https://github.com/atareao/watchbeat/issues/40) | este documento |

## 3. Cómo funciona un check (pipeline)

```
checker.check(monitor)
   │  HTTP/TCP/TLS resuelven con dns::resolve (Fase 1, caché TTL)
   ▼
CheckOutcome { status, cause: Option<FailureCause> }        (Fase 2)
   │
   ▼
attribution::attribute(status, cause, health.current())     (Fase 3)
   │  health = canario NetworkHealth (healthy | dns_degraded | down | unknown)
   ▼
status atribuido: up | warning | down | error | indeterminate
   │
   ├─ indeterminate → observabilidad: NO notifica, NO cambia was_up, NO penaliza uptime
   │
   ▼
transición de estado → notificación
   │  breaker (Fase 5): ¿tormenta? → una alerta agregada / supresión
   ▼
despacho a notificadores (telegram, matrix, ntfy, webhook, slack, discord, email, gotify…)
```

El **jitter** (Fase 4) actúa en el *cuándo*: cada monitor tiene un desplazamiento inicial
uniforme en `[0, intervalo)` y un jitter por ciclo de ±10 %, de modo que no cronan juntos.

## 4. Estados y semántica

| Estado | Significado | ¿Notifica? | ¿Cuenta uptime? |
|---|---|---|---|
| `up` | Servicio OK | — | Sí (numerador) |
| `warning` | OK con aviso (p. ej. latencia/TLS) | Según plantilla | Sí |
| `down` | Fallo **atribuible al target** | Sí | Sí (denominador) |
| `error` | Fallo **no atribuible / lado nuestro** | Sí (transición) | Sí |
| `indeterminate` | **No atribuible** (red/DNS del host) | **No** | **No** (ni num. ni den.) |

`indeterminate` es un estado de **primera clase**: se persiste, se muestra en la UI
(badge violeta + icono, doble canal color+forma), se distingue en la status page pública y
se excluye del uptime. Es observabilidad, no incidente.

## 5. Configuración

### Variables de entorno

| Variable | Default | Fase |
|---|---|---|
| `WATCHBEAT_DNS_UPSTREAMS` | `1.1.1.1,8.8.8.8,9.9.9.9` | 1 |
| `WATCHBEAT_RETENTION_DAYS` | `30` | — |

### Settings (tabla KV, editables desde la UI)

| Clave | Default | Fase |
|---|---|---|
| `default_template_down/latency/up/expiry` | (plantillas) | — |
| `storm_breaker_enabled` | `true` | 5 |
| `storm_threshold_percent` | `50` | 5 |
| `storm_window_seconds` | `10` | 5 |
| `storm_min_monitors` | `3` | 5 |
| `default_template_storm` | (vacío) | 5 |

## 6. Decisiones clave

Documentadas como ADR en [`docs/adr/`](adr/):

- [ADR-0001](adr/0001-atribucion-frente-a-reintentos.md) — atribución en lugar de reintentos.
- [ADR-0002](adr/0002-indeterminate-estado-primera-clase.md) — `indeterminate` como estado de primera clase.
- [ADR-0003](adr/0003-dns-propio-vs-dns-del-host.md) — DNS propio con caché.
- [ADR-0004](adr/0004-jitter-frente-a-interval-alineado.md) — jitter en lugar de interval alineado.
- [ADR-0005](adr/0005-breaker-por-episodio.md) — breaker con dedupe por episodio.

## 7. Métricas antes / después

| Métrica | Antes | Después |
|---|---|---|
| Falsos `down` por corte de DNS del host | Todos los monitores con DNS externo | 0 (caché TTL + `indeterminate`) |
| Alertas por una caída de infraestructura | N (una por monitor) | 1 agregada (breaker) |
| Checks disparados en el mismo segundo | 8/8 con `interval=60` | Desfasados (jitter) |
| Causa de un fallo | `down`/`error` genérico | `error_kind` tipado (10 causas) |
| Uptime durante un corte del host | Penalizado | Excluido (`indeterminate`) |

> Las cifras "antes" provienen de la incidencia real en `co1` (ver `HANDOFF.md`). Las
> "después" son el comportamiento garantizado por diseño y verificado en tests; la
> medición cuantitativa en producción queda como seguimiento.

## 8. Despliegue y rollback

- **Sin migraciones destructivas**: la única migración es `ALTER TABLE checks ADD COLUMN
  error_kind` (idempotente, aditiva).
- Todo es **activo por defecto**; el breaker se puede desactivar desde la UI.
- Rollback = revertir el PR de la fase correspondiente; los datos nuevos (`error_kind`,
  `indeterminate`) se ignoran sin romper.

## 9. Documentación por fase

- [robust-dns-resolver.md](robust-dns-resolver.md) — Fase 1
- [failure-cause-taxonomy.md](failure-cause-taxonomy.md) — Fase 2
- [network-attribution.md](network-attribution.md) — Fase 3
- [scheduler-jitter.md](scheduler-jitter.md) — Fase 4
- [alert-storm-breaker.md](alert-storm-breaker.md) — Fase 5
- [checks-perf.md](checks-perf.md) — rendimiento de queries (trabajo previo relacionado)
