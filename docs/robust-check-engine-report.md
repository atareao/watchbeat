# Informe final — Robust Check Engine (PLAN-002)

- **Issue paraguas**: [#34](https://github.com/atareao/watchbeat/issues/34)
- **Plan**: `plans/PLAN-002-ARCHIVED.md`
- **Periodo**: fases 1–6, PRs #42–#47
- **Fecha de cierre**: 2026-11-14

## Resumen ejecutivo

WatchBeat confundía las caídas de **infraestructura del host** (DNS/red) con caídas
**reales de los servicios**, generando falsos positivos y tormentas de alertas. El
**Robust Check Engine** resuelve el problema en cinco capas complementarias —DNS propio,
taxonomía de causas, atribución con canario, jitter y breaker— más una fase de cierre
documental. El resultado: un fallo de red del host ya **no** alerta como si el servicio
estuviera caído, y una caída en cascada genera **una** alerta en lugar de N.

## Qué se ha realizado

| Fase | Entrega | PR |
|---|---|---|
| 1 · DNS propio | `dns.rs`: caché TTL + fallback multi-upstream + `DnsError`; integrado en reqwest/TCP/TLS | #42 |
| 2 · Taxonomía | `FailureCause` + `error_kind` persistido; `status_for(cause)` unificado | #43 |
| 3 · Atribución | `netcheck.rs` (canario) + `attribution.rs`; estado `indeterminate` en backend y frontend | #44 |
| 4 · Jitter | `jitter.rs`: desplazamiento inicial + ±10 % por ciclo; bucle secuencial anti-ráfagas | #45 |
| 5 · Breaker | `breaker.rs`: detección de tormenta, alerta agregada, dedupe por episodio; pestaña "Tormenta" | #46 |
| 6 · Cierre | Doc global, ADR, README/CHANGELOG/env/guía, informe, cierre de issues | #47 |

## Por qué (decisiones clave)

- **Atribución, no reintentos** ([ADR-0001](adr/0001-atribucion-frente-a-reintentos.md)):
  un reintento enmascara el síntoma; la atribución dice *de quién* es el fallo, de forma
  inmediata y determinista.
- **`indeterminate` de primera clase** ([ADR-0002](adr/0002-indeterminate-estado-primera-clase.md)):
  ni "caído" ni "oculto"; observabilidad sin incidente.
- **DNS propio** ([ADR-0003](adr/0003-dns-propio-vs-dns-del-host.md)): la caché TTL absorbe
  los microcortes del DNS del host.
- **Jitter** ([ADR-0004](adr/0004-jitter-frente-a-interval-alineado.md)): rompe el
  *thundering herd* sin cambiar el intervalo configurado.
- **Breaker por episodio** ([ADR-0005](adr/0005-breaker-por-episodio.md)): una alerta por
  tormenta, con dedupe por monitor único y unión de notificadores.

## Métricas antes / después

| Métrica | Antes | Después |
|---|---|---|
| Falsos `down` por corte de DNS del host | Todos los monitores con DNS externo | 0 (caché TTL + `indeterminate`) |
| Alertas por una caída de infraestructura | N (una por monitor) | 1 agregada |
| Checks en el mismo segundo | 8/8 con `interval=60` | Desfasados (jitter) |
| Causa de un fallo | `down`/`error` genérico | `error_kind` (10 causas) |
| Uptime durante un corte del host | Penalizado | Excluido (`indeterminate`) |

> "Antes" = incidencia real en `co1` (ver `HANDOFF.md`). "Después" = comportamiento
> garantizado por diseño y verificado en tests; la medición cuantitativa en producción
> queda como seguimiento.

## Calidad

- **Tests**: 238 unit (lib) + 9 api_keys + 45 db_integration + 3 run_check_route, todos
  deterministas y sin red externa (upstreams, canario y webhooks inyectados).
- **Lint/format**: `cargo clippy --all-targets --all-features -- -D warnings` y
  `cargo fmt --check` limpios; `npx tsc --noEmit` y `npm run build` en verde.
- **Auditorías**: `@rust-reviewer` y `@react-reviewer` en cada fase; en la Fase 5 se
  corrigieron 2 hallazgos Mayores (flapping, tormenta silenciosa) y 1 regresión de orden.

## Riesgos residuales y follow-ups

- **Issue #41** ✅ **resuelto** (hotfix PR #48): se instala `ring` como `CryptoProvider`
  por defecto al arrancar, evitando el panic de los monitores TLS.
- Intervalo del canario de red fijo a 30 s (configurable en el futuro).
- Estado del breaker no expuesto en la API/dashboard (observabilidad futura).
- Sin deduplicación de consultas DNS en vuelo (trade-off conocido).

## Enlaces

- Visión global: [robust-check-engine.md](robust-check-engine.md)
- Docs por fase: [DNS](robust-dns-resolver.md) · [taxonomía](failure-cause-taxonomy.md) ·
  [atribución](network-attribution.md) · [jitter](scheduler-jitter.md) ·
  [breaker](alert-storm-breaker.md)
- ADR: [docs/adr/](adr/)
