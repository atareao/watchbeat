# Tasks — Notificador Valet

## Backend — módulo notifier/valet.rs

- [x] RED: Escribir tests tokio (TcpListener) en `notifier/valet.rs`:
      - POST a `{base_url}/api/notifications` con body JSON (message + level + origin + author)
      - Header `Authorization: Bearer <ingest_token>` presente
      - Header `Content-Type: application/json`
      - Error en 400/401/500 → Err("Valet API error: ...")
      - Conexión rechazada → Err
      - Mensaje >4096 bytes → truncado a 4096
- [x] GREEN: Implementar `send_valet_notification(base_url, ingest_token, origin, author, level, message)` en `notifier/valet.rs`
- [x] Verificar: ejecutar los tests nuevos (rojo antes, verde después) + `cargo check`

## Backend — registro en notifier/mod.rs

- [x] Añadir `pub mod valet;`
- [x] GREEN: Crear struct `ValetNotifier` + impl `NotifierTrait` (config: base_url, ingest_token, origin; author = monitor.name; level por defecto `info`)
- [x] GREEN: Añadir arm `"valet"` en `notifier_for()`
- [x] Verificar: `cargo check` + tests verdes

## Backend — dispatch en scheduler.rs

- [x] Renombrar `if let Some((_notif_type, message))` → `(notif_type, message)` para disponer del evento
- [x] GREEN: Añadir arm `"valet"` en el match dispatch (línea ~450):
      - base_url e ingest_token requeridos; origin default "WatchBeat", author = monitor.name
      - level según notif_type: down→critical, latency→warning, expiry→warning, up→info
      - llamar `notifier::valet::send_valet_notification(...)`
- [x] Verificar: `cargo check` + tests verdes (`cargo test`)

## Backend — test dispatch en routes/notifiers.rs

- [x] GREEN: Añadir arm `"valet"` en `send_test_notification` (level="info", message fijo de test)
- [x] Verificar: `cargo check` + tests verdes

## Backend — verificación global

- [x] `just check` (fmt + clippy -D warnings) limpio
- [x] `cargo test` completo verde

## Frontend — Settings.tsx

- [x] Añadir `{ value: 'valet', label: 'Valet' }` a `NOTIFIER_TYPES`
- [x] Añadir `valet: [...]` a `CONFIG_FIELDS` (base_url requerida, ingest_token password, origin opcional; author NO configurable)
- [x] Verificar: `npx tsc --noEmit` y `npm run build` OK

## Docs

- [x] Actualizar `README.md` con el nuevo tipo de notificador Valet (+ mención de ingest token / Notifications API)
