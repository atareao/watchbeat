# ADR-0001 · Atribución de fallos en lugar de reintentos

- **Estado**: aceptado
- **Fecha**: 2026-11-14
- **Contexto**: PLAN-002 (Robust Check Engine), Fase 3

## Contexto

Ante los falsos positivos por microcortes de red/DNS del host, la solución habitual es
**reintentar** el check (o exigir N confirmaciones) antes de declarar `down`. WatchBeat ya
tenía `confirmations_required`/`failed_checks`.

## Decisión

**No** usar reintentos ni debounce como mecanismo de atribución. En su lugar, **atribuir**
cada fallo combinando su causa (`FailureCause`) con la salud de nuestra propia red
(canario `NetworkHealth`), produciendo el estado `indeterminate` cuando el fallo no es
atribuible al target.

## Razones

- Un reintento es un **parche**: enmascara el síntoma (retrasa la alerta) sin decir *de
  quién* es el fallo. Un corte de 5 s puede superar cualquier número razonable de
  reintentos.
- La detección debe ser **inmediata** (una sola pasada por check); la decisión se corrige
  por **atribución**, no por repetición.
- La atribución es **determinista y testeable** (tabla pura `classify`), mientras que los
  reintentos introducen latencia y no determinismo.
- `confirmations_required` se conserva para su propósito legítimo (evitar flapping del
  target), pero **no** se usa para atribuir.

## Consecuencias

- Un fallo durante un corte del host se marca `indeterminate` (no alerta, no penaliza
  uptime) en lugar de reintentarse.
- Se necesita un canario de red fiable (ver ADR-0003 y `netcheck`).
- Verdad simplificada: durante un corte del host, un `down` real del target también se
  marca `indeterminate` (no se puede distinguir). Es el comportamiento deseado.
