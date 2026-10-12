# homepage-api Specification

## Purpose
Expone el estado y uptime de los monitores de una status page como JSON público, para que dashboards externos como gethomepage (widget `customapi`) puedan consumirlo con polling simple. El acceso puede protegerse opcionalmente con una API key configurable.

## Requirements

### Requirement: Endpoint público JSON GET /status/{slug}/api.json

When a request hits `GET /status/{slug}/api.json` for an existing **public** status page, the server SHALL respond `200` with `Content-Type: application/json` and a JSON object shaped as `{"status", "title", "updated_at", "monitors":[...]}`. The global `status` SHALL be computed from the same logic as the HTML public page (`down` if any listed monitor is down, `up` if all are up, else `unknown`). Each entry in `monitors` SHALL contain `id`, `name`, `target`, `status`, `response_time_ms`, `uptime_7d`, `uptime_30d`, and `last_checked_at` from the monitor summaries. Only monitors present in the page's `monitors` list SHALL be included. The route SHALL be registered in the public (JWT-free) router.

#### Scenario: Consulta de una status page pública
- **WHEN** a client GETs `/status/{slug}/api.json` for a status page with `public: true`
- **THEN** the response is `200` JSON with `status`, `title`, `updated_at`, and a `monitors` array limited to that page's monitored ids
- **AND** each monitor includes `id`, `name`, `target`, `status`, `response_time_ms`, `uptime_7d`, `uptime_30d`, `last_checked_at`

#### Scenario: Status page no existe
- **WHEN** a client GETs `/status/{slug}/api.json` for a non-existent slug
- **THEN** the response is `404` with an error message
- **AND** no JSON payload is returned

#### Scenario: Status page privada
- **WHEN** a client GETs `/status/{slug}/api.json` for a status page with `public: false`
- **THEN** the response is `404` (page not revealed), matching the HTML public page behaviour
- **AND** no monitor data is leaked

#### Scenario: Cálculo del estado global
- **WHEN** at least one listed monitor has `last_status == "down"`
- **THEN** `status` is `"down"`
- **WHEN** all listed monitors are `"up"` and at least one exists
- **THEN** `status` is `"up"`
- **WHEN** monitors exist but none has a known status
- **THEN** `status` is `"unknown"`

### Requirement: API key opcional configurable

The public endpoint `GET /status/{slug}/api.json` SHALL stop reading the `homepage_api_key` setting. Instead, it SHALL load all keys from the `api_keys` table: if the list is empty, the endpoint SHALL behave as fully public for public status pages; if one or more keys exist, the request SHALL present one of them via the `x-api-key` header (priority) or the `?api_key=` query param, and the presented value SHALL match (constant-time) ANY configured key; otherwise `401 Unauthorized`. The status page must still be `public: true` regardless of keys (private/non-existent → `404`). API keys SHALL be managed via the new protected `/api/api-keys` routes and stored in the `api_keys` table.

#### Scenario: API key no configurada
- **WHEN** the `api_keys` table is empty and the page is public
- **THEN** any request to the JSON endpoint is served without authentication

#### Scenario: API key configurada y presente
- **WHEN** one or more keys exist and the request includes a matching key (`x-api-key` header or `api_key` query param)
- **THEN** the endpoint is served normally (page must still be public)

#### Scenario: API key configurada y ausente/incorrecta
- **WHEN** at least one key exists and the request omits the key or sends one that matches no configured key
- **THEN** the response is `401 Unauthorized`
- **AND** no monitor data is returned

#### Scenario: API key no exime de page pública
- **WHEN** keys exist and a request presents a correct key for a page with `public: false`
- **THEN** the response is still `404` (page not revealed)
