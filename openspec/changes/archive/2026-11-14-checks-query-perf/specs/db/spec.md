## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Estadísticas del planner

Tras ejecutar las migraciones, `init_db` SHALL ejecutar `ANALYZE` para que el
planificador de SQLite disponga de estadísticas actualizadas de las tablas e
índices.

#### Scenario: ANALYZE tras la migración
Given una base de datos recién abierta
When `Database::open` termina
Then existe la tabla interna `sqlite_stat1`
And contiene al menos una fila para el índice `idx_checks_timeline`
