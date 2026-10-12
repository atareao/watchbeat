# api-keys Specification

## Purpose
Permite gestionar múltiples API keys (cada una con un nombre identificador único y una clave secreta UUID inmutable) desde la UI de Ajustes, y usarlas para proteger el endpoint público JSON de status pages.

## Requirements

### Requirement: Tabla api_keys y modelo

The system SHALL store API keys in a new `api_keys` table with columns `id` (TEXT PRIMARY KEY, UUID), `name` (TEXT NOT NULL UNIQUE), `api_key` (TEXT NOT NULL UNIQUE), `created_at` (TEXT), `updated_at` (TEXT). The domain model SHALL expose `ApiKey { id, name, api_key, created_at, updated_at }` and a `sqlx::FromRow` `ApiKeyRow` with a `From` conversion. Both `name` and `api_key` SHALL be unique and non-null. The `name` is the human-readable keyword identifying the consumer application; the `api_key` is the generated UUID secret, always auto-generated on creation and immutable afterwards.

#### Scenario: La tabla api_keys se crea al arrancar
- **WHEN** the database initializes in `Database::open`
- **THEN** the `api_keys` table exists with the specified schema
- **AND** `name` and `api_key` columns enforce UNIQUE and NOT NULL

#### Scenario: Convertir ApiKeyRow a ApiKey
- **GIVEN** an `ApiKeyRow` with all fields
- **WHEN** converted via `From<ApiKeyRow>`
- **THEN** the resulting `ApiKey` holds the same field values

### Requirement: CRUD protegido de API keys

The system SHALL expose protected routes under `/api/api-keys`: `GET` lists all keys, `POST` creates a key (requiring a non-null, unique `name`; the server SHALL auto-generate the `api_key` UUID and `id` UUID), `PUT /{id}` updates ONLY the `name` (must remain non-null and unique; the `api_key` UUID SHALL NOT change), and `DELETE /{id}` removes the key. All routes SHALL be behind the existing JWT auth middleware. Duplicate `name` or `api_key` SHALL be rejected.

#### Scenario: Listar API keys
- **WHEN** a client GETs `/api/api-keys`
- **THEN** the response is `200` with the full list of `ApiKey` objects

#### Scenario: Crear una API key
- **WHEN** a client POSTs `/api/api-keys` with a non-empty `name` that is not already used
- **THEN** the response is `201` with the created `ApiKey` including a server-generated UUID in both `id` and `api_key`

#### Scenario: Crear una API key con nombre vacío
- **WHEN** a client POSTs `/api/api-keys` with an empty or whitespace-only `name`
- **THEN** the response is `400` and no key is created

#### Scenario: Crear una API key con nombre duplicado
- **WHEN** a client POSTs `/api/api-keys` with a `name` already present in the table
- **THEN** the response is `409 Conflict` and no key is created

#### Scenario: Editar solo el nombre
- **WHEN** a client PUTs `/api/api-keys/{id}` with a new unique non-empty `name`
- **THEN** the `name` is updated
- **AND** the `api_key` UUID is unchanged
- **AND** `updated_at` is refreshed

#### Scenario: Editar con nombre duplicado
- **WHEN** a client PUTs `/api/api-keys/{id}` with a `name` belonging to another key
- **THEN** the response is `409 Conflict` and no change is applied

#### Scenario: Eliminar una API key
- **WHEN** a client DELETEs `/api/api-keys/{id}`
- **THEN** the key is removed and the response is `200`/`204`

#### Scenario: Operar sobre un id inexistente
- **WHEN** a client PUTs or DELETEs `/api/api-keys/{id}` for a non-existent id
- **THEN** the response is `404`

### Requirement: UI de gestión de API keys

The Settings page SHALL include a new "API Keys" tab listing all keys (Name, masked key with a copy button, Created) with actions to Add (opens a modal for the name; the server returns the generated UUID which is shown and copyable), Edit (only the name, the key UUID is read-only), and Delete (with confirmation). Name uniqueness (non-empty, no duplicates) SHALL be validated on create and edit.

#### Scenario: La pestaña API Keys lista las keys
- **WHEN** the user opens the API Keys tab
- **THEN** a table shows each key's name, a masked key value that can be copied, and its created date

#### Scenario: Crear una API key desde la UI
- **WHEN** the user clicks Add, enters a unique non-empty name and confirms
- **THEN** the key is created and the generated UUID is shown to be copied

#### Scenario: Editar el nombre desde la UI
- **WHEN** the user edits a key and changes only its name to a unique non-empty value
- **THEN** the name updates and the UUID remains unchanged (read-only in the form)

#### Scenario: Eliminar una API key desde la UI
- **WHEN** the user confirms deletion of a key
- **THEN** the key is removed from the list
