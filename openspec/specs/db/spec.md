# db Specification

## Purpose
TBD - created by archiving change checks-perf. Update Purpose after archive.

## Requirements

### Requirement: Índice DESC para queries de timeline

```sql
CREATE INDEX IF NOT EXISTS idx_checks_timeline ON checks(monitor_id, checked_at DESC, status)
```

SHALL existir en la base de datos tras la migración.

#### Scenario: get_checks_filtered usa el nuevo índice DESC
Given un monitor con 100k checks y el índice idx_checks_uptime ASC existente
When se ejecuta `get_checks_filtered` con status `['up','down']` y ORDER BY DESC
Then la query usa `idx_checks_timeline` (index scan, no sort) en lugar de idx_checks_uptime
And el tiempo es < 10ms para LIMIT 20

#### Scenario: get_timeline usa el nuevo índice DESC
Given el mismo monitor con 100k checks
When se ejecuta `get_timeline(monitor_id, since)` con ORDER BY ASC
Then la query puede usar idx_checks_timeline en orden inverso sin sort

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
