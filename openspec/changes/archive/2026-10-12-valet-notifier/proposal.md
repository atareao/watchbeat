# Proposal: Notificador Valet

## Why

Valet es el asistente personal de IA (self-hosted) que expone una **Notifications API** (`POST /api/notifications`) para que cualquier aplicación externa pueda empujar alertas a su inbox, autenticado con un **ingest token** por aplicación. WatchBeat (uptime monitor) necesita poder notificar caídas, latencia y expiración de certificados directamente a la bandeja de entrada de Valet, igual que ya hace con Telegram, Matrix, ntfy, webhook, Slack, Discord, email y Gotify.

## What Changes

- Nuevo **tipo de notificador** `valet` en WatchBeat, con campos de configuración:
  - `base_url` (URL raíz de Valet, p.ej. `http://valet:3000`) — requerido.
  - `ingest_token` (el token `Authorization: Bearer` de ingesta de Valet) — requerido.
  - `origin` (opcional, ≤64 bytes; default = nombre de la key en Valet).
  - `author` (opcional, ≤128 bytes; default = nombre de la key en Valet).
- **Dispatch** (`scheduler.rs` + `notifier/mod.rs` + test en `routes/notifiers.rs`): al notificar, hacer `POST {base_url}/api/notifications` con header `Authorization: Bearer <ingest_token>`, `Content-Type: application/json`, y body `{ message, level, origin?, author? }`.
- **Mapeo de `level`** según el tipo de evento que genera WatchBeat:
  - `down` → `critical`
  - `latency` → `warning`
  - `expiry` → `warning`
  - `up` → `info`
- El `message` se envía truncado a ≤4096 bytes (límite de Valet).
- Frontend: nuevo tipo `Valet` en `Settings.tsx` (`NOTIFIER_TYPES` + `CONFIG_FIELDS`) con los campos anteriores.

**No es breaking:** es un tipo de notificador nuevo y aditivo; los existentes no cambian. No requiere migración de DB (la config vive en `config_json` de la tabla `notifiers`), ni nuevos endpoints.

## Capabilities

### New Capabilities
- `notifiers`: tipo de notificador `valet` que envía alertas a la Notifications API de Valet (ingest con token Bearer, `POST /api/notifications`, nivel `critical`/`warning`/`info`).

## Impact

- **Código**: `backend/src/notifier/valet.rs` (nuevo módulo `send_valet_notification` + tests), `backend/src/notifier/mod.rs` (registrar módulo + `ValetNotifier` + arm en `notifier_for`), `backend/src/scheduler.rs` (arm `"valet"` en el dispatch, reutilizar `notif_type` para el `level`), `backend/src/routes/notifiers.rs` (arm "valet" en `send_test_notification`).
- **Router**: ninguno (no hay rutas nuevas).
- **Datos**: ninguno (sin migración).
- **Dependencias**: ninguna nueva — `reqwest` y `serde_json` ya presentes.
- **Frontend**: `frontend/src/pages/Settings.tsx` (nuevo tipo en `NOTIFIER_TYPES` + `CONFIG_FIELDS`).
- **Docs**: `README.md` (mención del nuevo tipo de notificador).
