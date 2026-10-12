# Tasks — api-keys (API keys múltiples)

## 1. RED — Tests fallidos

- [x] 1.1 Tests de modelo: `ApiKeyRow → ApiKey` via From (models.rs)
- [x] 1.2 Tests DB (db.rs): crear key con UUID, nombre duplicado → error, `api_key` duplicado → error, listar, actualizar solo nombre (UUID inmutable), borrar, lookups por `api_key`
- [x] 1.3 Tests handler `api-keys` (routes/api_keys.rs): list, create (name vacío → 400, duplicado → 409, ok → 201 con UUID), update name (duplicado → 409, inexistente → 404), delete (inexistente → 404), y que el UUID no cambia en update
- [x] 1.4 Tests endpoint público (routes/status_pages.rs): sin keys → público; con 1+ keys y key correcta → servido; key ausente/incorrecta → 401; varias keys válidas; key correcta pero page privada → 404
- [x] 1.5 Verificar RED vía `cargo test`

## 2. GREEN — Implementación mínima

- [x] 2.1 `backend/src/models.rs`: structs `ApiKey` + `ApiKeyRow` + From
- [x] 2.2 `backend/src/db.rs`: tabla `api_keys` + métodos list/create/update-name/delete/get-by-key/name-unique
- [x] 2.3 `backend/src/routes/api_keys.rs`: handlers CRUD (generar UUID con `uuid` crate)
- [x] 2.4 `backend/src/routes/mod.rs`: registro del módulo `api_keys` + rutas protegidas `/api/api-keys`
- [x] 2.5 `backend/src/routes/status_pages.rs`: modificar `public_page_json` para validar contra la lista de `api_keys` (elimina lectura de `homepage_api_key`); ajustar `api_key_authorized` a conjunto de keys; comparación constante-tiempo
- [x] 2.6 Asegurar `uuid` en Cargo.toml si no está
- [x] 2.7 `cargo test` hasta verde + `cargo check`

## 3. GREEN (frontend)

- [x] 3.1 `frontend/src/api/http.ts`: tipo `ApiKey` + `fetchApiKeys`/`createApiKey`/`updateApiKey`/`deleteApiKey`
- [x] 3.2 `frontend/src/pages/Settings.tsx`: nueva pestaña "API Keys" (tabla + modal añadir/editar solo nombre + eliminar + mostrar/copiar UUID)

## 4. REFACTOR — Limpieza

- [x] 4.1 `cargo clippy -- -D warnings` y `cargo fmt --check` hasta limpio
- [x] 4.2 `npx tsc --noEmit` y/o `npm run lint` en frontend
- [x] 4.3 Eliminar referencias muertas a `homepage_api_key` en código y README (actualizar doc)
- [x] 4.4 `cargo test` completo verde, sin regresiones en `/status/{slug}` HTML

## 5. Consolidación

- [x] 5.1 Verificación manual del endpoint contra una status page pública con y sin keys
- [x] 5.2 `openspec validate api-keys`
- [x] 5.3 `openspec archive api-keys` y merge a `development`
