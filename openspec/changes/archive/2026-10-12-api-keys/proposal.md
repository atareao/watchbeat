# Proposal: API Keys múltiples para protección del endpoint público

## Why

Actualmente la protección del endpoint público `GET /status/{slug}/api.json` se basa en una única setting `homepage_api_key` guardada en la tabla `settings`. Esto impide dar credenciales distintas a cada consumidor (gethomepage, scripts, dashboards). Se necesita un modelo multi-key gestionable: cada API key tiene un nombre (palabra clave que identifica la aplicación/consumidor, no nulo y único) y una clave secreta UUID (única e inmutable). Toda la gestión se hace desde la UI en una nueva pestaña de Ajustes.

## What Changes

- Nueva tabla `api_keys` con columnas: `id` (UUID, PK), `name` (TEXT NOT NULL UNIQUE), `api_key` (TEXT NOT NULL UNIQUE — el UUID secreto), `created_at`, `updated_at`.
- Nuevo módulo de rutas protegidas `api_keys`:
  - `GET /api/api-keys` → lista todas las keys.
  - `POST /api/api-keys` → crea una key; el servidor genera el UUID secreto; requiere `name` no nulo y único.
  - `PUT /api/api-keys/{id}` → actualiza SOLO el `name` (la clave UUID es inmutable). Nombre único.
  - `DELETE /api/api-keys/{id}` → elimina la key.
- El endpoint público `GET /status/{slug}/api.json` deja de leer `homepage_api_key`: ahora consulta la tabla `api_keys`. Si no hay ninguna key → endpoint público. Si hay una o más → la request debe presentar una key (`x-api-key` header o `?api_key=` query) que coincida con cualquiera de las configuradas (comparación constante-tiempo); si falta o no coincide → `401`.
- Se elimina el uso de la setting `homepage_api_key` (decisión: eliminar sin migrar los datos existentes).
- Frontend: nueva pestaña "API Keys" en Ajustes con tabla (nombre, clave (enmascarada + copiar), creada) y CRUD (añadir / editar solo nombre / eliminar). El UUID se muestra al crear y se puede copiar, pero no se puede editar en la edición.

**BREAKING:** El comportamiento del endpoint público cambia si alguien dependía de la setting `homepage_api_key` única (deja de ser válida). Es un cambio intencionado y aceptado.

## Capabilities

### New Capabilities
- `api-keys`: Gestión CRUD de múltiples API keys (nombre + UUID secreto) y su uso para proteger el endpoint público JSON.

### Modified Capabilities
- `homepage-api`: El endpoint `GET /status/{slug}/api.json` pasa de validar la setting única `homepage_api_key` a validar contra el conjunto de keys de la tabla `api_keys`.

## Impact

- **Código**: `backend/src/models.rs` (nuevo struct `ApiKey` + `ApiKeyRow` + From), `backend/src/db.rs` (nueva tabla `api_keys` + métodos CRUD + lookups), `backend/src/routes/api_keys.rs` (nuevo módulo), `backend/src/routes/mod.rs` (registro módulo + rutas), `backend/src/routes/status_pages.rs` (modificar `public_page_json` y `api_key_authorized`).
- **Router**: 4 rutas protegidas nuevas bajo `/api/api-keys`.
- **Datos**: nueva tabla `api_keys`. La setting `homepage_api_key` queda sin usar (no se migra).
- **Dependencias**: `uuid` para generar los UUID (verificar si ya está en Cargo.toml; si no, añadir). `subtle` y `sha2` ya presentes.
- **Frontend**: `frontend/src/pages/Settings.tsx` (nueva pestaña) y `frontend/src/api/http.ts` (funciones + tipo ApiKey).
