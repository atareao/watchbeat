# Proposal

**Issue:** [#40](https://github.com/atareao/watchbeat/issues/40)
**Plan:** `plans/PLAN-002.md` — Robust Check Engine (Fase 6 · cierre)

## Why

Las fases 1–5 están implementadas, auditadas y mergeadas, pero el conocimiento está
disperso en seis `docs/<feature>.md` y en los change proposals archivados. Falta una
**visión global** que explique *qué* se hizo, *por qué* (razones y decisiones) y *cómo*
funciona en conjunto, además de los ADR de las decisiones clave, la actualización de la
documentación de usuario (README, CHANGELOG, env example, guía de self-hoster) y el
**informe final** con métricas antes/después. También hay que **cerrar** los issues
#35–#40 y el paraguas #34, y archivar el plan.

## What Changes

- **`docs/robust-check-engine.md`** — documento global: problema, arquitectura de las 6
  fases, flujo de un check, estados, configuración y enlaces a los docs por fase.
- **`docs/adr/`** — ADR de las decisiones clave:
  - descartar reintentos en favor de atribución;
  - `indeterminate` como estado de primera clase;
  - DNS propio vs DNS del host;
  - jitter en lugar de interval alineado;
  - breaker por episodio.
- **`README.md`** — sección de resiliencia/atribución + enlaces a la doc global.
- **`CHANGELOG.md`** — entrada de la release con las 5 fases.
- **`watchbeat.env.example`** — `WATCHBEAT_DNS_UPSTREAMS` y notas de settings del breaker.
- **`docs/self-hoster-guide.md`** — sección de troubleshooting de falsos positivos.
- **Informe final** — resumen ejecutivo + métricas antes/después.
- **Cierre**: cerrar #35–#40 y #34; renombrar `plans/PLAN-002.md` → `PLAN-002-ARCHIVED.md`.

## Capabilities

### New Capabilities
_(ninguna — cambio puramente documental)_

### Modified Capabilities
_(ninguna)_

## Impact

- Solo documentación y metadatos. **Sin cambios de código** ni de comportamiento.
- `skip_specs: true` en el change (no hay deltas de spec).
