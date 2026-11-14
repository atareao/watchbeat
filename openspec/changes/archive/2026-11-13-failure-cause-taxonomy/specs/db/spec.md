# Database — Causa de fallo

## ADDED Requirements

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
