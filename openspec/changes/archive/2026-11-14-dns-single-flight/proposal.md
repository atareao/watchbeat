# Proposal

**Issue:** [#68](https://github.com/atareao/watchbeat/issues/68)
**Plan:** `plans/PLAN-011.md`

## Why

`DnsResolver::resolve_all` (`backend/src/dns.rs`) no coordina llamadas concurrentes: dos
resoluciones del mismo host en cache-miss lanzan **dos** consultas al upstream (thundering
herd). Con varios checks concurrentes del mismo host (HTTP + TCP + TLS, o varios monitores),
el número de consultas crece con el número de llamadas, añadiendo latencia y carga al upstream
y al propio proceso. Es el bloque `a` de #62.

## What Changes

- Se añade coordinación *single-flight* en `DnsResolver`: mientras una consulta para un host
  normalizado está en vuelo, las llamadas concurrentes comparten esa única consulta y su
  resultado (éxito o error). Los aciertos de caché siguen sirviéndose sin coordinación.
- Se añaden tests de concurrencia con un upstream falso retardado: N llamadas concurrentes →
  exactamente una consulta; mismo resultado para todas; error compartido; hosts distintos
  independientes; acierto de caché sin consulta.

## Capabilities

### Added Capabilities

- `dns`: las resoluciones concurrentes de un mismo host en cache-miss SHALL compartir una
  única consulta a upstream y su resultado.

## Impact

- `backend/src/dns.rs` (`DnsResolver` + `#[cfg(test)] mod tests`).
- Sin cambios de API pública ni de esquema.
