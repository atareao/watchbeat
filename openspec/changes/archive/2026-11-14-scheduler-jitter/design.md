# Design

## Context

Ver `proposal.md` — Why. Hoy (`scheduler.rs:267`):
```rust
let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
interval.tick().await; // consume el primer tick inmediato
```
El primer tick de `tokio::time::interval` es **inmediato**, así que todos los
monitores arrancados juntos ejecutan su primer check en el mismo instante; y como
comparten el mismo periodo, siguen alineados.

## Goals / Non-Goals

**Goals**
- Desincronizar los ticks de los monitores (romper el *thundering herd*).
- Evitar la re-convergencia a lo largo del tiempo.
- Mantener la propiedad anti-ráfagas y el comportamiento observable (salvo el
  desfase).
- Jitter **testeable** y determinista en tests.

**Non-Goals**
- Cambiar el `interval_seconds` configurado.
- Rate limiting / breaker (Fase 5).
- Jitter en el canario de red (`netcheck`), que es un único task (no un herd).
- Priorizar/ordenar monitores.

## Decisions

- **D1 · Desplazamiento inicial uniforme.** Antes del primer check, cada monitor
  espera un offset uniforme en `[0, intervalo)`. Esto desincroniza a los monitores
  con el mismo intervalo de forma **permanente** (mantienen su desfase). Con
  `intervalo = 0`/1 s, el offset se acota para no bloquear.
- **D2 · Jitter por ciclo ±10 %.** Entre checks, la espera es
  `intervalo ± 10 %` (uniforme), lo que evita que converjan de nuevo tras
  reinicios, `Update` o `reload`. Cota inferior dura: la espera nunca es < 1 s
  (evita martillear el target).
- **D3 · Bucle secuencial con `sleep`.** Se sustituye `tokio::time::interval` por
  un bucle `run_check(); sleep(jittered(intervalo));`. Al ser **secuencial**, un
  check que se alarga retrasa el siguiente en vez de acumular ticks → no hay
  ráfagas (subsume `MissedTickBehavior::Skip`). Además simplifica la aplicación
  del jitter por ciclo.
- **D4 · RNG inyectable.** El cálculo del jitter se aísla tras una abstracción
  (trait `Jitter` o función que recibe un `Rng`/closure). En producción se usa
  `rand`; en tests, un RNG determinista (valores fijos) para asertar rangos y
  reproducibilidad. Sin esto, los tests serían frágiles.
- **D5 · No cambia el `interval_seconds`.** El jitter altera el **delay**, no el
  valor configurado del monitor (que se persiste y se muestra tal cual).
- **D6 · Independiente de la Fase 3.** El jitter es ortogonal a la atribución y
  al canario: no toca `run_monitor_check` ni el estado `indeterminate`.

## Risks / Trade-offs

- [El offset inicial retrasa el primer check de un monitor] → hasta un intervalo
  (p. ej. 60 s). Aceptable en un sistema de monitorización; el importe se acota
  para intervalos pequeños. Alternativa descartada: repartir solo en una ventana
  corta (no rompe el herd de N grande).
- [Dependencia nueva `rand`] → ya transitiva en el árbol (vía otras crates);
  confinada en el helper de jitter.
- [Cambia el ritmo de los checks] → el comportamiento observable (resultado del
  check) no cambia; solo el *cuándo*.

## Migration Plan

- Sin migración. Despliegue normal. Rollback = revertir el PR.
- Efecto visible: tras el arranque, los monitores ejecutan su primer check
  repartido a lo largo de un intervalo en lugar de todos a la vez.

## Open Questions

- ¿Aplicar el offset inicial también a monitores creados por el usuario en
  caliente, o solo al cargar? En esta fase: se aplica siempre (uniforme y acotado);
  no cambia las specs.
