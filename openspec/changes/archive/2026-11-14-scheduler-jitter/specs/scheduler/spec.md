# Scheduler — Jitter de ticks

## ADDED Requirements

### Requirement: Los ciclos de los monitores SHALL estar desincronizados (jitter)

El scheduler SHALL incorporar **jitter** al ciclo de cada monitor para evitar que
varios monitores ejecuten su check en el mismo instante (*thundering herd*). El
jitter SHALL tener dos componentes:

- un **desplazamiento inicial** uniforme en `[0, intervalo)` aplicado antes del
  primer check, que desincroniza monitores con el mismo intervalo;
- un **jitter por ciclo** de ±10 % del intervalo aplicado a la espera entre
  checks, que evita la re-convergencia.

El jitter NO SHALL modificar el `interval_seconds` configurado del monitor ni el
resultado observable de los checks (solo el *cuándo*). El cálculo del jitter SHALL
ser testeable con una fuente de aleatoriedad inyectable.

#### Scenario: Monitores con el mismo intervalo no arrancan a la vez
Given 8 monitores con el mismo intervalo, todos arrancados simultáneamente
When el scheduler los despacha
Then sus primeros checks no ocurren todos en el mismo segundo
And quedan repartidos a lo largo del intervalo

#### Scenario: El desplazamiento inicial está dentro del intervalo
Given un monitor con intervalo `I`
When se calcula su desplazamiento inicial
Then el desplazamiento está en `[0, I)`

#### Scenario: El jitter por ciclo está en ±10 %
Given un monitor con intervalo `I`
When se calcula la espera entre dos checks
Then la espera está en `[0.9·I, 1.1·I]`

#### Scenario: Cota inferior de la espera
Given un monitor con un intervalo muy pequeño
When se calcula la espera entre checks
Then la espera nunca es menor que el mínimo configurado (1 s)

#### Scenario: El intervalo configurado no cambia
Given un monitor con `interval_seconds = 60`
When se aplica el jitter
Then el `interval_seconds` configurado y persistido sigue siendo 60

#### Scenario: Cálculo reproducible con aleatoriedad inyectada
Given una fuente de aleatoriedad determinista inyectada
When se calcula el jitter varias veces
Then los valores son reproducibles y caen dentro del rango esperado

### Requirement: El ciclo con jitter SHALL preservar la propiedad anti-ráfagas

Un check que tarda más que el intervalo NO SHALL provocar una ráfaga de checks
posteriores: la espera SHALL contarse **después** de completar el check.

#### Scenario: Un check lento no genera ráfaga
Given un monitor cuyo check tarda más que su intervalo
When el check termina
Then el siguiente check espera el delay con jitter completo
And no se ejecuta una ráfaga de checks de recuperación
