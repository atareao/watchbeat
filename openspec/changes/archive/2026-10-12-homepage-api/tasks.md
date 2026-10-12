# Tasks — homepage-api (Custom API JSON para gethomepage)

## 1. RED — Tests fallidos para el endpoint JSON

- [x] 1.1 Añadir tests unitarios al handler `public_page_json` (routes/status_pages.rs): page pública devuelve JSON con `status`/`title`/`updated_at`/`monitors`, y cada monitor incluye los campos esperados
- [x] 1.2 Test: slug inexistente → `404`
- [x] 1.3 Test: page privada (`public: false`) → `404`
- [x] 1.4 Test: cálculo del estado global (down / up / unknown)
- [x] 1.5 Test: API key no configurada → endpoint servido sin autenticación (page pública)
- [x] 1.6 Test: API key configurada + key correcta (header `x-api-key` y query `api_key`) → servido
- [x] 1.7 Test: API key configurada + key ausente/incorrecta → `401`
- [x] 1.8 Test: API key correcta pero page privada → `404`
- [x] 1.9 Verificar que los tests fallan (RED) vía `cargo test`

## 2. GREEN — Implementación mínima

- [x] 2.1 Implementar `public_page_json` en `backend/src/routes/status_pages.rs`
- [x] 2.2 Registrar la ruta `GET /status/{slug}/api.json` en el router público de `backend/src/routes/mod.rs` (junto a `/status/{slug}`)
- [x] 2.3 Leer el setting `homepage_api_key` (vía `db.get_setting`) y aplicar la lógica de API key (header/query, 401, page pública)
- [x] 2.4 Ejecutar `cargo test` hasta verde y `cargo check`

## 3. REFACTOR — Limpieza

- [x] 3.1 Ejecutar `cargo clippy -- -D warnings` y `cargo fmt --check` hasta limpio
- [x] 3.2 Revisar que no hay código duplicado entre `public_page` y `public_page_json` (extraer helper de cálculo de estado y de autenticación por API key si procede)
- [x] 3.3 Confirmar que `/status/{slug}` (HTML) sigue intacto

## 4. Consolidación

- [x] 4.1 Verificación manual del endpoint contra una status page pública
- [x] 4.2 `openspec validate homepage-api`
- [x] 4.3 `openspec archive homepage-api` y merge a `development`
