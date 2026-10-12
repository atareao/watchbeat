# Change Proposal: Emisión de eventos SSE incremental (solo cambios) + resync ante Lagged

## Intent

Reducir el tráfico SSE y eliminar la pérdida silenciosa de eventos haciendo dos cambios:
1. **Emitir solo eventos relevantes** (cambio de estado o breach de latencia), no uno por cada check.
2. **Detectar `broadcast::RecvError::Lagged`** en el `sse_handler` y emitir un evento `resync` para que el frontend haga un `refresh()` inmediato en lugar de esperar el poll de 60s.

Se mantiene la capacidad de broadcast en `256` (se demuestra suficiente para 200 monitores @ 60s).

## Problem

**Tráfico desperdiciado.** `run_monitor_check` emite un evento SSE por cada check ejecutado (`scheduler.rs` L339-351), incluso cuando el estado no ha cambiado. Con 200 monitores @ 60s, eso son ~3.33 eventos/segundo emitidos continuamente, la gran mayoría irrelevantes para la UI (el mismo estado, latencia dentro de umbral).

**Pérdida silenciosa de eventos.** `broadcast` descarta eventos para un suscriptor lento (o que se reconecta) devolviendo `RecvError::Lagged(n)`. En `sse_handler` (`routes/mod.rs` L140-144), ese error se convierte en `Event::default().data("")` — un evento vacío que el frontend descarta. El cliente **no sabe que perdió datos** y depende del fallback poll de 60s del Dashboard para volver a estar al día.

## Scope

**Backend (2 archivos):**
- `backend/src/scheduler.rs`
- `backend/src/routes/mod.rs`

**Frontend (2 archivos):**
- `frontend/src/hooks/useSse.ts`
- `frontend/src/pages/Dashboard.tsx` (y `MonitorDetail.tsx` si aplica)

**Backend — scheduler.rs:**
- Condicionar la emisión SSE a que el check sea un cambio relevante para la UI: `status_changed` (cambio up/down/error/warning) o breach de latencia.
- Mantener la guarda `event_tx.receiver_count() > 0`.
- Mantener el payload JSON actual (`type`, `monitor_id`, `monitor_name`, `status`, `response_time_ms`, `error_message`, `checked_at`).

**Backend — routes/mod.rs (sse_handler):**
- Al recibir `Err(RecvError::Lagged(_))` en el `BroadcastStream`, emitir un evento `{"type":"resync"}` en lugar de un evento vacío.

**Frontend — useSse.ts:**
- Tipar el evento con dos variantes: `check` y `resync`.
- Para `resync`: invocar un callback de "resync" (nuevo, opcional) en lugar del `onEvent`.
- Reconexión con backoff exponencial (2s → 4s → 8s → máx 30s), y reset a 2s tras una conexión estable.

**Frontend — Dashboard.tsx / MonitorDetail.tsx:**
- Conectar el evento `resync` a un `refresh()` / recarga inmediata de datos.
- El fallback poll de 60s se conserva como red de seguridad.

## Impact

Backend:
- Emisiones SSE con 200 monitores estables @ 60s: de ~3.33 msg/s a **~0 msg/s** (solo emite cuando cambia algo).
- Buffer broadcast 256: absorbe ráfagas de 200 eventos simultáneos sin `Lagged` (margen confirmado).
- Latencia de UI tras pérdida de conexión: de **hasta 60s** (fallback poll) a **inmediata** (evento `resync`).

Frontend:
- Menos re-fetches redundantes en `Dashboard.tsx` y `MonitorDetail.tsx`.
- Reconexión sin martilleo del backend en caídas largas (backoff).

## Non-goals

- No cambiar la capacidad `SSE_CHANNEL_CAPACITY` (se mantiene en 256).
- No cambiar la persistencia en DB (muestreo time-based intacto).
- No cambiar el dispatch de notificaciones.
- No emitir eventos de CRUD de monitores/notifiers/status-pages (se deja como cambio futuro).
- No añadir keepalive/heartbeat del stream SSE.
- No eliminar el fallback poll de 60s.

## Test strategy

- Unit tests en `scheduler.rs`: un monitor estable (varios checks consecutivos sin cambio) **no** produce evento SSE tras el primero; un cambio de estado sí produce evento.
- Unit tests del `sse_handler`: un `RecvError::Lagged` produce un evento `{"type":"resync"}` en el stream.
- Frontend: verificación manual / revisión del mapeo de eventos en `useSse.ts`.
