# Database — Uptime con estado indeterminate

## ADDED Requirements

### Requirement: El uptime SHALL excluir los checks indeterminate

El cálculo de uptime SHALL contar en el numerador solo los checks con status
`up` y SHALL excluir del **denominador** los checks con status `indeterminate`,
de modo que un fallo no atribuible al target no penalice su uptime.

#### Scenario: Indeterminate no penaliza el uptime
Given un monitor con 8 checks `up` y 2 checks `indeterminate` en el periodo
When se calcula su uptime
Then el uptime es 100%

#### Scenario: Down sí penaliza el uptime
Given un monitor con 8 checks `up` y 2 checks `down` en el periodo
When se calcula su uptime
Then el uptime es 80%

#### Scenario: Ventana solo con indeterminate
Given un monitor cuyos checks del periodo son todos `indeterminate`
When se calcula su uptime
Then el uptime no se calcula (sin datos atribuibles)

### Requirement: El timeline NO SHALL pintar como down un bucket sin fallos atribuibles

`get_timeline_buckets` SHALL excluir los checks `indeterminate` del denominador
del porcentaje de uptime y SHALL emitir `dominant_status = "indeterminate"` cuando
el bucket no contenga checks atribuibles (solo `indeterminate`).

#### Scenario: Bucket con up e indeterminate
Given un bucket con checks `up` e `indeterminate`
When se calcula el bucket
Then el porcentaje de uptime excluye los `indeterminate`
And el `dominant_status` no es `indeterminate` si hay checks atribuibles

#### Scenario: Bucket solo con indeterminate
Given un bucket cuyos únicos checks son `indeterminate`
When se calcula el bucket
Then el `dominant_status` es `indeterminate`
And NO es `down`

#### Scenario: Bucket con caída real
Given un bucket con checks `down` mayoritarios
When se calcula el bucket
Then el `dominant_status` es `down`

### Requirement: El dashboard SHALL exponer el recuento de monitores indeterminate

El endpoint de estado del dashboard SHALL exponer un recuento de monitores en
estado `indeterminate`, de modo que el total cuadre con la suma de categorías.

#### Scenario: Total cuadra con las categorías de los monitores con check
Given N monitores **con al menos un check** repartidos entre `up`, `down` e `indeterminate`
When se consulta el estado del dashboard
Then el total es la suma de los tres recuentos
And existe un recuento `indeterminate_monitors`
And los monitores sin check o deshabilitados no alteran ese invariante
