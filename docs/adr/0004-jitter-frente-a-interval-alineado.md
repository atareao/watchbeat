# ADR-0004 · Jitter en lugar de `tokio::time::interval` alineado

- **Estado**: aceptado
- **Fecha**: 2026-11-14
- **Contexto**: PLAN-002, Fase 4

## Contexto

Todos los monitores usaban `tokio::time::interval`, cuyo primer tick es **inmediato**. Con
N monitores del mismo intervalo, todos ejecutaban su check en el mismo segundo
(*thundering herd*): picos simultáneos de conexiones, escrituras SQLite y tráfico de red.

## Decisión

Añadir **jitter** al ciclo de cada monitor:

- **desplazamiento inicial** uniforme en `[0, intervalo)` antes del primer check;
- **jitter por ciclo** de ±10 % del intervalo (cota mínima 1 s).

Se sustituye `tokio::time::interval` por un **bucle secuencial con `sleep`**.

## Razones

- El desplazamiento inicial **desincroniza** de forma permanente a los monitores del mismo
  intervalo.
- El jitter por ciclo evita la **re-convergencia** tras reinicios o recargas.
- El bucle secuencial preserva la propiedad **anti-ráfagas** (un check lento retrasa el
  siguiente; subsume `MissedTickBehavior::Skip`).
- El RNG y el reloj son **inyectables** → tests deterministas sin sleeps reales.

## Consecuencias

- El primer check de un monitor se retrasa hasta un intervalo (aceptable en monitorización).
- El `interval_seconds` configurado **no cambia** (solo el *cuándo*).
- Dependencia `rand` (ya transitiva).
