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

The endpoint SHALL read a per-install API key from settings (key `homepage_api_key`). If that setting is defined and non-empty, every request to `GET /status/{slug}/api.json` SHALL be required to present it, either via the `x-api-key` header or the `?api_key=` query param; a missing or wrong key SHALL yield `401 Unauthorized`. If the setting is empty/absent, the endpoint SHALL behave as fully public for public status pages. The status page still must be public regardless of the API key. The API key SHALL be set via the existing protected settings endpoints (`POST /api/settings`) and stored in the existing `settings` table.

#### Scenario: API key no configurada
- **WHEN** the `homepage_api_key` setting is empty/absent and the page is public
- **THEN** any request to the JSON endpoint is served without authentication

#### Scenario: API key configurada y presente
- **WHEN** `homepage_api_key` is set and the request includes the matching key (`x-api-key` header or `api_key` query param)
- **THEN** the endpoint is served normally (page must still be public)

#### Scenario: API key configurada y ausente/incorrecta
- **WHEN** `homepage_api_key` is set and the request omits the key or sends a wrong one
- **THEN** the response is `401 Unauthorized`
- **AND** no monitor data is returned

#### Scenario: API key no exime de page pública
- **WHEN** `homepage_api_key` is set and a request presents the correct key for a page with `public: false`
- **THEN** the response is still `404` (page not revealed)
