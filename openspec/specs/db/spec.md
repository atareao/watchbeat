# db Specification

## Purpose
TBD - created by archiving change checks-perf. Update Purpose after archive.

## Requirements

### Requirement: Índice DESC para queries de timeline

```sql
CREATE INDEX IF NOT EXISTS idx_checks_timeline
    ON checks(monitor_id, checked_at DESC, status, response_time_ms)
```

SHALL existir en la base de datos tras la migración. El índice SHALL ser
**cubriente** para las queries de timeline (que leen `status` y
`response_time_ms`) y SHALL sustituir a los índices redundantes
`idx_checks_monitor` e `idx_checks_uptime`, que SHALL eliminarse.

#### Scenario: get_checks_filtered usa el nuevo índice DESC
Given un monitor con 100k checks y el índice idx_checks_uptime ASC existente
When se ejecuta `get_checks_filtered` con status `['up','down']` y ORDER BY DESC
Then la query usa `idx_checks_timeline` (index scan, no sort) en lugar de idx_checks_uptime
And el tiempo es < 10ms para LIMIT 20

#### Scenario: get_timeline usa el nuevo índice DESC
Given el mismo monitor con 100k checks
When se ejecuta `get_timeline(monitor_id, since)` con ORDER BY ASC
Then la query puede usar idx_checks_timeline en orden inverso sin sort

#### Scenario: get_checks_filtered usa el índice DESC sin sort
Given un monitor con 100k checks
When se ejecuta `get_checks_filtered` con status `['up','down']` y ORDER BY DESC
Then el plan de ejecución usa `idx_checks_timeline`
And NO aparece `USE TEMP B-TREE FOR ORDER BY`
And el tiempo es < 10ms para LIMIT 20

#### Scenario: get_timeline_buckets usa el índice como covering
Given un monitor con 100k checks
When se ejecuta `get_timeline_buckets` sobre 30 días
Then el plan de ejecución usa `idx_checks_timeline` como COVERING INDEX
And NO hay table lookup sobre `checks`
And el tiempo es < 100ms

#### Scenario: calculate_uptime usa el índice como covering
Given un monitor con 100k checks
When se ejecuta `calculate_uptime` sobre 30 días
Then el plan de ejecución usa `idx_checks_timeline` como COVERING INDEX

#### Scenario: índices redundantes eliminados
Given una base de datos migrada
When se consulta `sqlite_master` por los índices de `checks`
Then existe `idx_checks_timeline`
And NO existen `idx_checks_monitor` ni `idx_checks_uptime`

### Requirement: Bucketing en SQL en vez de en Rust

`get_timeline_buckets` SHALL usar GROUP BY en SQL en lugar de bajar todas las filas a memoria y filtrarlas en Rust.

#### Scenario: timeline_buckets devuelve ~60 bloques
Given un monitor con 21k checks en 14 días
When se llama a `get_timeline_buckets`
Then devuelve ~60 buckets (nunca más de 70)
And cada bucket tiene `timestamp_ms`, `count`, `up_count`
And el tiempo total es < 100ms (vs 3.5s actual)

#### Scenario: timeline_buckets con monitor inexistente
Given un monitor_id que no existe en la DB
When se llama a `get_timeline_buckets`
Then devuelve lista vacía

#### Scenario: timeline_buckets con rango vacío
Given un monitor con checks, pero `since` posterior a todos los checks
When se llama a `get_timeline_buckets`
Then devuelve lista vacía

### Requirement: Columna error_kind en checks

La tabla `checks` SHALL tener una columna `error_kind` (TEXT, nullable) que
almacene la causa tipada del fallo, y `insert_check` SHALL persistirla.

#### Scenario: Migración idempotente
Given una base de datos existente sin la columna
When se inicializa la DB
Then se añade la columna `error_kind` a `checks`
And volver a inicializar no produce error

#### Scenario: Persistir la causa de un fallo
Given un check que falla con causa `connect_refused`
When se guarda con `insert_check`
Then la fila almacena `error_kind = 'connect_refused'`

#### Scenario: Check correcto sin causa
Given un check correcto sin causa
When se guarda con `insert_check`
Then la fila almacena `error_kind = NULL`

#### Scenario: Lectura de la causa
Given una fila con `error_kind` no nulo
When se lee el check
Then el resultado expone `error_kind` con ese valor

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

### Requirement: Estadísticas del planner

Tras ejecutar las migraciones, `init_db` SHALL ejecutar `ANALYZE` para que el
planificador de SQLite disponga de estadísticas actualizadas de las tablas e
índices.

#### Scenario: ANALYZE tras la migración
Given una base de datos recién abierta
When `Database::open` termina
Then existe la tabla interna `sqlite_stat1`
And contiene al menos una fila para el índice `idx_checks_timeline`
