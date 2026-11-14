# PLAN-011 — Dedupe de consultas DNS en vuelo (#68)

**Issue:** [#68](https://github.com/atareao/watchbeat/issues/68)
**Estado:** en curso
**Creado:** 2026-11-14

## Objetivo

Eliminar el thundering herd en la resolución DNS: hoy `DnsResolver::resolve_all`
(`backend/src/dns.rs`) no coordina llamadas concurrentes, así que N resoluciones del mismo
host en cache-miss lanzan N consultas al upstream. Añadir *single-flight*: una única consulta
en vuelo por host, compartida por las llamadas concurrentes (resultado incluido).

## Alcance

1. Coordinación en vuelo en `DnsResolver` (mapa de consultas en curso por host normalizado).
2. Tests de concurrencia con upstream falso retardado: N llamadas → 1 consulta; resultado
   compartido; error compartido; hosts distintos independientes; acierto de caché sin consulta.

## Tareas

- [ ] RED: tests de concurrencia que fallan (N consultas en lugar de 1)
- [ ] GREEN: single-flight en `DnsResolver`
- [ ] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde
- [ ] Doc `docs/dns-single-flight.md`
- [ ] `openspec archive dns-single-flight`
- [ ] PR a `development` con `Closes #68`
- [ ] Renombrar a `plans/PLAN-011-ARCHIVED.md` al cerrar

## Criterios de aceptación

- N resoluciones concurrentes del mismo host disparan una sola consulta al upstream.
- El resultado (éxito o error) se comparte entre las llamadas concurrentes.
- Sin cambios de API pública ni de esquema.
- `cargo clippy -- -D warnings` y `cargo fmt --check` limpios.
