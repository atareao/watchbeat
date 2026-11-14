# Scheduler tick jitter

## Qué es

Los ciclos de check de cada monitor ya no arrancan todos en el mismo instante: el
scheduler aplica **jitter** al ciclo de cada monitor para evitar el *thundering
herd* (N monitores que cronan en el mismo segundo → picos simultáneos de
conexiones, escrituras SQLite y tráfico de red, y fallos simultáneos ante un
microcorte de DNS/red del host).

- Issue: [#38](https://github.com/atareao/watchbeat/issues/38)
- Plan: `plans/PLAN-002-ARCHIVED.md` — Robust Check Engine, Fase 4
- Change proposal: `openspec/changes/scheduler-jitter/`

## Ficheros implicados

| Fichero | Papel |
|---|---|
| `backend/src/jitter.rs` | **Nuevo.** `initial_offset`, `next_delay`, `MIN_DELAY`, trait `Cycle` y el bucle `jittered_cycle`. Tests deterministas (RNG y reloj inyectados) |
| `backend/src/scheduler.rs` | `monitor_task_inner` usa `jittered_cycle`; `MonitorCycle` implementa `Cycle` (un check + el estado en memoria del monitor) |
| `backend/src/lib.rs` | Exporta el módulo `jitter` |
| `backend/Cargo.toml` | Dependencia `rand` 0.10 (ya presente de forma transitiva; solo se declara) |

## Cómo funciona

Antes (`tokio::time::interval`) el primer tick era **inmediato**, así que todos los
monitores arrancados juntos ejecutaban su primer check en el mismo segundo y, al
compartir periodo, seguían alineados. Ahora:

1. **Desplazamiento inicial (D1)**: antes del primer check, cada monitor espera un
   offset uniforme en `[0, intervalo)`. Con el mismo intervalo, dos monitores
   dibujan offsets distintos y **mantienen** su desfase.
2. **Jitter por ciclo (D2)**: entre checks la espera es uniforme en
   `[0.9·I, 1.1·I]`, con **cota inferior dura de 1 s** (`MIN_DELAY`), para que no
   vuelvan a converger tras un reinicio o un `Update`.
3. **Bucle secuencial (D3)**: `check(); sleep(jittered(I))`. La espera se cuenta
   **después** de completar el check, así que un check lento retrasa el siguiente
   en vez de acumular ticks de recuperación: esto **subsume**
   `MissedTickBehavior::Skip` (que ya no se usa).

El cálculo es puro respecto al intervalo y al RNG (`next_delay(interval, rng)`):
no depende de cuánto tardó el check anterior, que es lo que garantiza la
propiedad anti-ráfagas.

## Configuración

Sin variables de entorno nuevas. El `interval_seconds` del monitor **no cambia**
(D5): el jitter altera el *delay*, no el valor configurado ni el persistido, ni la
API. El efecto visible es que, tras el arranque, los primeros checks se reparten a
lo largo de un intervalo en lugar de ejecutarse todos a la vez.

## Decisiones y follow-ups

- **RNG inyectable (D4)**: `initial_offset`/`next_delay` son genéricos sobre
  `rand::Rng`, así que los tests usan `StdRng::seed_from_u64` y las aserciones son
  deterministas. En producción se usa `rand::make_rng::<StdRng>()` (`rand::rng()`
  es `!Send` y el task del monitor corre en el runtime multihilo).
- **Reloj inyectado**: el bucle recibe el `sleep` como parámetro, de modo que el
  escenario anti-ráfagas se prueba con un reloj falso y sin esperas reales.
- **Fuera de alcance (D6)**: la atribución y el canario de red (`netcheck`) no se
  tocan; el canario es un único task, no un herd.
- Follow-up: rate limiting / breaker (Fase 5).
