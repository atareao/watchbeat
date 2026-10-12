# Proposal: API JSON para integración con gethomepage (Custom API)

## Why

gethomepage (https://gethomepage.dev) puede mostrar el estado de servicios self-hosted mediante su widget `customapi`, que hace polling a una URL JSON y mapea campos. WatchBeat ya expone status pages públicas en `/status/{slug}` (HTML) y tiene toda la información necesaria (`MonitorSummary`: `last_status`, `uptime_7d`, `uptime_30d`, `response_time_ms`). Falta exponer esa misma información en **formato JSON** para que un dashboard como gethomepage pueda consumirla sin fricción, con protección opcional por API key.

## What Changes

- Añadir endpoint **público** `GET /status/{slug}/api.json` que devuelve el estado de una status page como JSON estructurado.
- Respuesta con:
  - `status` global de la page (`up` / `down` / `unknown`), coherente con el cálculo usado en `public_page`.
  - `title` y `updated_at` de la page.
  - `monitors`: array de objetos con `id`, `name`, `target`, `status`, `response_time_ms`, `uptime_7d`, `uptime_30d`, `last_checked_at`.
- Filtro de monitores por la lista `page.monitors`, igual que en `public_page`.
- **API key opcional** configurable (setting `homepage_api_key`): si está definida, se exige vía `x-api-key` header o `?api_key=` query param (ausente/incorrecta → `401`); si está vacía, el endpoint es público. En cualquier caso la page debe ser pública (`public: true`), si no → `404`.
- No se toca el HTML existente de `/status/{slug}` ni el middleware de autenticación de otras rutas.

**BREAKING:** Ninguno. Es aditivo (nuevo endpoint, nueva ruta pública + settings key opcional).

## Capabilities

### New Capabilities
- `homepage-api`: Endpoint JSON público (con API key opcional) para que gethomepage (widget `customapi`) lea el estado y uptime de los monitores de una status page.

### Modified Capabilities
- (none)

## Impact

- **Código**: `backend/src/routes/status_pages.rs` (nuevo handler `public_page_json` + registro de ruta en `backend/src/routes/mod.rs`).
- **Router**: nueva ruta pública `GET /status/${slug}/api.json` (se añade a la lista de rutas que saltan JWT).
- **Datos**: `Database::list_status_pages` y `get_monitor_summaries` ya existentes — sin cambios de esquema DB. Se lee `homepage_api_key` de la tabla `settings` existente vía `get_setting`.
- **Dependencias**: ninguna nueva (serde_json ya disponible).
- **Frontend**: no afectado.
