# Notifiers — Valet

## ADDED Requirements

### Requirement: Tipo de notificador `valet`

El backend SHALL reconocer un tipo de notificador `valet`. No requiere migración
de DB; la configuración se almacena en `config_json` de la tabla `notifiers`.

Campos de `config_json`:

| Campo | Tipo | Requerido | Default | Descripción |
|-------|------|-----------|---------|-------------|
| `base_url` | string | Sí | — | URL raíz de Valet (sin trailing slash), ej. `http://valet:3000` |
| `ingest_token` | string | Sí | — | Token de ingesta (`val_...`), enviado como `Authorization: Bearer` |
| `origin` | string | No | `"WatchBeat"` | Identificador del origen (≤64 bytes) |

El campo `author` NO es configurable: se deriva del nombre del monitor
(`monitor.name`) en el dispatch del scheduler.

#### Scenario: Happy path — envía notificación correctamente

```
Given un notificador Valet con base_url, ingest_token y origin configurados
When se envía una notificación desde el monitor "Servidor WEB" con level="critical"
  Y message="Monitor down: Servidor WEB"
Then la función hace POST a {base_url}/api/notifications
  Con header Authorization: Bearer <ingest_token>
  Con header Content-Type: application/json
  Con body JSON: { "message": "Monitor down: Servidor WEB", "level": "critical", "origin": "WatchBeat", "author": "Servidor WEB" }
Y recibe un 201; devuelve Ok(())
```

#### Scenario: Error de API — Valet devuelve 400/401/500

```
Given un notificador Valet configurado
When la API de Valet responde con 400 (message required) o 401 (unauthorized) o 500
Then la función devuelve Err("Valet API error: <status> — <body>")
```

#### Scenario: Conexión rechazada — Valet no está corriendo

```
Given un notificador Valet configurado con base_url=http://127.0.0.1:1
When se intenta enviar la notificación
Then la función devuelve Err con error de conexión
```

#### Scenario: Mensaje truncado a 4096 bytes

```
Given un mensaje de >4096 bytes
When se envía la notificación
Then el mensaje se trunca a 4096 bytes antes de enviar el POST
```

### Requirement: Scheduler dispatch del tipo `valet`

El dispatch de notificaciones en `scheduler.rs` SHALL manejar el tipo `valet`
usando el nombre del monitor como `author`.

#### Scenario: Mapeo de eventos → level

```
Cuando el tipo de evento es "down"     → level = "critical"
Cuando el tipo de evento es "latency"  → level = "warning"
Cuando el tipo de evento es "expiry"   → level = "warning"
Cuando el tipo de evento es "up"       → level = "info"
```

#### Scenario: Dispatch usa monitor.name como author

```
Given un monitor "Servidor WEB" que emite una notificación hacia un notificador tipo "valet" habilitado
And config_json contiene base_url e ingest_token
When el scheduler despacha la notificación
Then se renombra _notif_type a notif_type en `if let Some((_notif_type, message))`
And se llama send_valet_notification(base_url, ingest_token, origin, monitor.name, level, message)
```

### Requirement: Test dispatch del tipo `valet`

El endpoint de prueba de notificador (`POST /api/notifiers/{id}/test`) SHALL
soportar el tipo `valet`. Como no hay monitor asociado, el `author` usa el valor
por defecto `"WatchBeat"`.

#### Scenario: Test notification envía level info

```
Given un notificador tipo "valet" con base_url e ingest_token
When se envía una notificación de prueba
Then send_test_notification llama send_valet_notification con level="info", author="WatchBeat" y message="Test notification from WatchBeat"
```
