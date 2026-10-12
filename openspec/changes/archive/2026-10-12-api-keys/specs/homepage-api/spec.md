# homepage-api Specification

## MODIFIED Requirements

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
