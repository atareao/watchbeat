# 📗 Guía para Self-Hosters — WatchBeat

> Cómo instalar, configurar y mantener WatchBeat en tu VPS.

---

## Índice

1. [Requisitos previos](#1-requisitos-previos)
2. [Arquitectura y concepto](#2-arquitectura-y-concepto)
3. [Proveedor OIDC](#3-proveedor-oidc)
4. [Instalación con Docker Compose](#4-instalación-con-docker-compose)
5. [Instalación con Podman](#5-instalación-con-podman)
6. [Instalación manual (sin Docker)](#6-instalación-manual-sin-docker)
7. [Configuración del reverse proxy](#7-configuración-del-reverse-proxy)
8. [Primeros pasos](#8-primeros-pasos)
9. [Creación de monitores](#9-creación-de-monitores)
10. [Configuración de notificadores](#10-configuración-de-notificadores)
11. [Heartbeats](#11-heartbeats)
12. [Status Pages](#12-status-pages)
13. [Plantillas de notificación](#13-plantillas-de-notificación)
14. [Backup y recuperación](#14-backup-y-recuperación)
15. [Exportación e importación de configuración](#15-exportación-e-importación-de-configuración)
16. [Mantenimiento](#16-mantenimiento)
17. [Monitorización de WatchBeat](#17-monitorización-de-watchbeat)
18. [Variables de entorno — referencia completa](#18-variables-de-entorno--referencia-completa)
19. [Solución de problemas](#19-solución-de-problemas)

---

## 1. Requisitos previos

### Hardware recomendado

| Recurso | Mínimo | Recomendado |
|---|---|---|
| CPU | 1 núcleo | 2 núcleos |
| RAM | 128 MB | 256 MB |
| Disco | 1 GB | 5 GB |
| SO | Linux (x86_64) | Linux (amd64 o arm64) |

Para ~50 monitores con checks cada 5 minutos, WatchBeat consume ~200 MB de RAM y genera ~300 MB de datos al mes con retención de 14 días.

### Software necesario

| Software | Versión mínima | Notas |
|---|---|---|
| Docker | 24+ | o Podman 4+ |
| Docker Compose | v2 | incluido en Docker |
| Reverse proxy | Cualquiera | Nginx, Caddy, Traefik, HAProxy |
| Proveedor OIDC | — | Authentik, Keycloak, Authelia, Google, etc. |

### Puertos

- `3055` — Puerto por defecto de WatchBeat (configurable con `PORT`)

---

## 2. Arquitectura y concepto

WatchBeat es un **monitor de uptime auto-hosteado**:

- **Un solo binario** que incluye el frontend SPA compilado dentro del ejecutable (gracias a `include_dir!`).
- **Base de datos SQLite** en modo WAL. Sin PostgreSQL, sin Redis, sin dependencias externas.
- **Scheduler interno** que cada ~15 segundos revisa los monitores, ejecuta los checks que tocan, y envía notificaciones si algo cambia.
- **Eventos en tiempo real** via Server-Sent Events (SSE) — el frontend se actualiza solo.
- **Consolidación de métricas** cada hora para que las gráficas de uptime a 7d, 30d, etc. sean instantáneas.

```
                    ┌──────────────────────┐
                    │   Proveedor OIDC     │
                    │  (Authentik, etc.)   │
                    └──────┬───────────────┘
                           │ autenticación
                           ▼
  Usuario ─── HTTPS ──► Reverse Proxy ───► WatchBeat :3055
                           │                    │
                           │              ┌─────┴─────┐
                           │              │  SQLite   │
                           │              │  (WAL)    │
                           │              └───────────┘
                           │
                    ┌──────┴──────┐
                    │  Internet   │
                    │  (checks)   │
                    └─────────────┘
```

---

## 3. Proveedor OIDC

WatchBeat **requiere** un proveedor OIDC. No tiene sistema de usuarios propio. Cada persona que abre WatchBeat se autentica contra tu proveedor OIDC.

### Opciones recomendadas

| Proveedor | Self-hosted | Comentario |
|---|---|---|
| **Authentik** | ✅ Sí | El más recomendado. Fácil, completo, actualizado. |
| **Keycloak** | ✅ Sí | Muy potente, más complejo. |
| **Authelia** | ✅ Sí | Ligero, integra con tu LDAP. |
| **Google Workspace** | ❌ No | Para equipos que ya usan Google. |
| **Azure AD** | ❌ No | Para entornos Microsoft. |
| **Okta** | ❌ No | SaaS, plan gratuito limitado. |
| **GitHub** | ❌ No | Si ya tienes cuenta. |
| **Dex** | ✅ Sí | Ligero, conéctalo a LDAP/GitHub/Google. |

### Configuración en Authentik (ejemplo)

1. En Authentik, ve a **Admin → Applications → Providers**.
2. Crea un **Provider** de tipo **OAuth2/OpenID Provider**:
   - **Client Type**: `Confidential`
   - **Redirect URIs/Origins (RegEx)**: `https://watchbeat.tudominio.com/auth/callback`
3. Crea una **Application**:
   - **Slug**: `watchbeat`
   - **Provider**: el que acabas de crear
4. Anota el **Client ID** y **Client Secret**.

### Redirect URI — ¡Cuidado! ⚠️

La `OIDC_REDIRECT_URL` debe coincidir **EXACTAMENTE** con la registrada en tu proveedor. El path siempre es `/auth/callback`. Cualquier diferencia (barra al final, http vs https, puerto omitido) hará que el login falle.

✅ Correctas:
- `http://localhost:3055/auth/callback` (desarrollo)
- `https://watchbeat.tudominio.com/auth/callback` (producción)

❌ Incorrectas:
- `http://localhost:3055/auth/callback/` (barra extra)
- `https://watchbeat.tudominio.com/Auth/Callback` (mayúsculas)

---

## 4. Instalación con Docker Compose

### 4.1. Preparar el directorio

```bash
mkdir -p /opt/watchbeat
cd /opt/watchbeat
```

### 4.2. Descargar los archivos

```bash
# Desde GitHub
wget https://raw.githubusercontent.com/atareao/watchbeat/main/compose.yml
wget https://raw.githubusercontent.com/atareao/watchbeat/main/watchbeat.env.example -O .env
wget https://raw.githubusercontent.com/atareao/watchbeat/main/Dockerfile
```

O clona el repositorio:

```bash
git clone https://github.com/atareao/watchbeat.git /opt/watchbeat
cd /opt/watchbeat
cp watchbeat.env.example .env
```

### 4.3. Configurar variables de entorno

Edita `.env`:

```bash
nano .env
```

Variables **obligatorias**:

```env
OIDC_ISSUER_URL=https://auth.tudominio.com/application/o/watchbeat/
OIDC_CLIENT_ID=watchbeat
OIDC_CLIENT_SECRET=tu-secreto-muy-largo
OIDC_REDIRECT_URL=https://watchbeat.tudominio.com/auth/callback
```

### 4.4. Arrancar

```bash
docker compose up -d
```

Comprueba que arranca:

```bash
docker compose logs -f
```

Deberías ver:

```
🚀 WatchBeat starting...
🔌 Checking connectivity...
✅ Internet connectivity OK
✅ OIDC provider reachable
📦 Database opened: /app/data/watchbeat.db
✅ OIDC discovery: https://auth.tudominio.com
🌐 WatchBeat en http://[::]:3055
```

### 4.5. Verificar healthcheck

```bash
docker compose ps
# El estado debe mostrar "healthy"
```

---

## 5. Instalación con Podman

Si usas Podman en lugar de Docker, el proceso es similar:

### Rootful (con systemd)

```bash
mkdir -p /opt/watchbeat && cd /opt/watchbeat
cp watchbeat.env.example .env
# Editar .env

# Construir la imagen
podman build -t watchbeat:latest .

# O usar el compose
podman-compose up -d
```

### Rootless (cuadrantes / Quadlet)

Si prefieres ejecutar con Podman Quadlet:

```properties
# ~/.config/containers/systemd/watchbeat.container
[Container]
Image=localhost/watchbeat:latest
ContainerName=watchbeat
PublishPort=3055:3055
Volume=watchbeat_data:/app/data
EnvironmentFile=/opt/watchbeat/.env

[Service]
Restart=always

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user start watchbeat
```

---

## 6. Instalación manual (sin Docker)

Si prefieres ejecutar el binario directamente en tu VPS:

### 6.1. Compilar desde fuente

```bash
# Clonar
git clone https://github.com/atareao/watchbeat.git /opt/watchbeat
cd /opt/watchbeat

# Compilar backend
cd backend
cargo build --release
strip target/release/watchbeat

# Compilar frontend
cd ../frontend
npm install
npm run build

# Volver al directorio raíz
cd ..
```

### 6.2. Crear usuario y directorios

```bash
sudo useradd -r -s /bin/false -d /opt/watchbeat watchbeat
sudo mkdir -p /opt/watchbeat/data
sudo chown -R watchbeat:watchbeat /opt/watchbeat
```

### 6.3. Crear archivo de entorno

```bash
sudo nano /opt/watchbeat/.env
```

### 6.4. Systemd service

```ini
# /etc/systemd/system/watchbeat.service
[Unit]
Description=WatchBeat - Self-hosted uptime monitor
After=network.target

[Service]
Type=simple
User=watchbeat
Group=watchbeat
WorkingDirectory=/opt/watchbeat
EnvironmentFile=/opt/watchbeat/.env
ExecStart=/opt/watchbeat/backend/target/release/watchbeat
Restart=always
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now watchbeat
```

---

## 7. Configuración del reverse proxy

WatchBeat sirve el frontend embebido (SPA), así que solo necesitas redirigir el tráfico al puerto 3055.

### 7.1. Caddy

```caddy
# /etc/caddy/Caddyfile
watchbeat.tudominio.com {
    reverse_proxy localhost:3055
}
```

Caddy maneja HTTPS automáticamente con Let's Encrypt.

### 7.2. Nginx

```nginx
# /etc/nginx/sites-available/watchbeat
server {
    listen 443 ssl;
    server_name watchbeat.tudominio.com;

    ssl_certificate /etc/letsencrypt/live/watchbeat.tudominio.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/watchbeat.tudominio.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3055;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Necesario para SSE
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
}
```

### 7.3. Traefik (Docker)

Si usas Traefik como reverse proxy, añade labels al servicio en `compose.yml`:

```yaml
services:
  watchbeat:
    # ... configuración existente ...
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.watchbeat.rule=Host(`watchbeat.tudominio.com`)"
      - "traefik.http.routers.watchbeat.entrypoints=websecure"
      - "traefik.http.services.watchbeat.loadbalancer.server.port=3055"
```

### 7.4. Consideraciones para SSE

Los eventos en tiempo real (SSE) requieren que el proxy no bufferice la respuesta:

- **Nginx**: `proxy_buffering off;`
- **Caddy**: Por defecto no bufferiza.
- **Traefik**: Por defecto no bufferiza.

---

## 8. Primeros pasos

### 8.1. Acceder a WatchBeat

Abre `https://watchbeat.tudominio.com` en tu navegador.

Serás redirigido a tu proveedor OIDC para iniciar sesión. Tras autenticarte, verás el Dashboard.

### 8.2. El Dashboard

El Dashboard muestra:

- **Estadísticas generales**: total de monitores, en línea, con problemas, latencia media.
- **Cuadrícula de monitores**: cada tarjeta muestra nombre, tipo, estado, uptime y última comprobación.
- **Búsqueda y filtros**: por nombre/target, tipo (HTTP, TCP, Ping, TLS, Heartbeat) y estado (UP, DOWN, Error).
- **Paginación**: configurable (10, 20, 50, 100 por página).

### 8.3. Tema oscuro

Usa el botón ☀️/🌙 en la cabecera para cambiar entre tema claro y oscuro. La preferencia se guarda en `localStorage`.

---

## 9. Creación de monitores

### 9.1. Monitor HTTP(S)

El más común. Verifica que una página web responde correctamente.

**Campos:**
- **Nombre**: identificador (ej: "Web principal")
- **Target**: URL completa (ej: `https://atareao.es`)
- **Intervalo**: cada cuántos minutos comprobar
- **Timeout**: tiempo máximo de espera (segundos)
- **Confirmaciones**: número de fallos consecutivos antes de marcar DOWN (evita falsos positivos)

**Configuración específica:**

| Campo | Descripción |
|---|---|
| **Método HTTP** | GET, HEAD o POST. HEAD es el más rápido (no descarga el body). |
| **Status esperado** | Código HTTP esperado. Por defecto acepta 2xx/3xx. |
| **Body esperado** | Texto que debe aparecer en la respuesta. |
| **Body es regex** | Interpreta el body esperado como regex. |
| **Días para expiry** | Días antes de la expiración del certificado TLS para alertar. |

### 9.2. Monitor TCP

Verifica que un puerto TCP está abierto y acepta conexiones.

- **Target**: `host:puerto` (ej: `atareao.es:443`)

### 9.3. Monitor Ping

Verifica respuesta ICMP.

- **Target**: IP o dominio (ej: `8.8.8.8`)

> ⚠️ El contenedor necesita `NET_RAW` capability para hacer ping. En `compose.yml` no está incluido por defecto. Si lo necesitas, añade:
> ```yaml
> cap_add:
>   - NET_RAW
> ```

### 9.4. Monitor TLS/SSL

Comprueba la fecha de expiración del certificado TLS.

- **Target**: host (sin `https://`), opcionalmente `host:puerto` (ej: `atareao.es`)

Alerta cuando quedan menos días de los configurados en `expiry_days`.

### 9.5. Umbral de latencia

Puedes definir un umbral de latencia (en ms). Cuando un monitor está UP pero supera el umbral, se envía una notificación de latencia alta. Útil para detectar degradaciones antes de que sean caídas.

### 9.6. Tags

Puedes añadir etiquetas a los monitores para organizarlos. Aunque actualmente no hay filtro por tags en la UI, los tags se incluyen en el contexto de las plantillas de notificación.

---

## 10. Configuración de notificadores

Los notificadores se gestionan desde **Settings → Notificadores**.

### 10.1. Crear un notificador

1. Ve a **Ajustes → Notificadores**.
2. Haz clic en **Añadir**.
3. Elige el tipo y rellena los campos.
4. Guarda.

### 10.2. Probar un notificador

Usa el botón **Probar** para enviar una notificación de prueba. Así verificas que la configuración es correcta antes de asociarlo a monitores.

### 10.3. Asociar notificadores a un monitor

Al crear o editar un monitor, puedes seleccionar un notificador en el campo correspondiente. Cada monitor puede tener múltiples notificadores.

### 10.4. Referencia de canales

| Canal | Campos | Ejemplo |
|---|---|---|
| **Telegram** | `bot_token`, `chat_id` | Crea un bot con [@BotFather](https://t.me/botfather) |
| **Matrix** | `homeserver_url`, `access_token`, `room_id` | Token desde Element o cliente Matrix |
| **ntfy** | `topic`, `server_url` (opc), `token` (opc) | Usa `https://ntfy.sh` o tu propio servidor |
| **Webhook** | `url`, `method` (opc), `headers` (opc) | POST a cualquier URL |
| **Slack** | `webhook_url` | Crea un Slack Webhook en tu espacio de trabajo |
| **Discord** | `webhook_url` | Crea un Webhook en la configuración del canal |
| **Email** | `smtp_host`, `smtp_port`, `username`, `password`, `from`, `to` | SMTP de tu proveedor o servidor de correo |
| **Gotify** | `server_url`, `app_token`, `priority` (opc) | Tu instancia de Gotify self-hosted |

---

## 11. Heartbeats

Los heartbeats son un tipo especial de monitor que no hace comprobaciones activas. En su lugar, **espera un pulso externo**.

### 11.1. Casos de uso

- **Backups**: el script de backup envía un pulso cuando termina.
- **CRON jobs**: un trabajo programado envía un pulso después de ejecutarse.
- **Servicios internos**: un servicio en tu red envía un pulso periódico.
- **IoT / dispositivos**: sensores que reportan su estado.

### 11.2. Cómo funciona

1. Creas un monitor de tipo **Heartbeat**.
2. WatchBeat genera un **token único** para ese monitor.
3. Tu servicio/envía un `POST` a `https://watchbeat.tudominio.com/api/heartbeat/{token}`
4. WatchBeat registra el pulso y actualiza `last_seen_at`.
5. Si el monitor no recibe pulso en el tiempo de **grace period**, se marca como DOWN.

### 11.3. Ejemplo con curl

```bash
curl -X POST https://watchbeat.tudominio.com/api/heartbeat/TOKEN_AQUI
```

### 11.4. Ejemplo con script bash

```bash
#!/bin/bash
# backup-completed.sh
# Envía pulso después de hacer el backup
rsync -avz /datos usuario@backup:/backup/
curl -s -o /dev/null -X POST https://watchbeat.tudominio.com/api/heartbeat/TOKEN_BACKUP
```

### 11.5. Grace period

El **grace period** es el tiempo máximo (en segundos) que WatchBeat espera entre pulsos antes de considerar el heartbeat como DOWN.

Ejemplo: si configuras 3600 segundos (1 hora), y tu script de backup se ejecuta cada día, WatchBeat marcará el heartbeat como DOWN si pasan más de 60 minutos sin pulso.

---

## 12. Status Pages

Las Status Pages son páginas públicas que muestran el estado de los monitores seleccionados. No requieren autenticación para verlas.

### 12.1. Crear una Status Page

1. Ve a **Ajustes → Status Pages**.
2. Haz clic en **Crear**.
3. Configura:
   - **Título**: nombre visible
   - **Slug**: identificador en la URL (solo minúsculas y guiones)
   - **Descripción**: texto opcional
   - **Monitores**: selecciona qué monitores mostrar
   - **Pública**: si está desactivado, la página solo es accesible con autenticación

### 12.2. URL pública

Si la status page es pública, estará disponible en:
`https://watchbeat.tudominio.com/status/{slug}`

### 12.3. Usos típicos

- **Página de estado pública** para tus clientes o usuarios.
- **Panel interno** mostrando solo servicios críticos.
- **Vista por equipo** o por proyecto.

---

## 13. Plantillas de notificación

WatchBeat usa [Jinja2](https://jinja.palletsprojects.com/) (motor `minijinja`) para personalizar los mensajes de notificación.

### 13.1. Variables disponibles

| Variable | Descripción |
|---|---|
| `{{ monitor_name }}` | Nombre del monitor |
| `{{ monitor_type }}` | Tipo de monitor (http, tcp, ping, tls, heartbeat) |
| `{{ target }}` | Target del monitor (URL, host, etc.) |
| `{{ status }}` | Estado actual (up, down, error) |
| `{{ previous_status }}` | Estado anterior |
| `{{ response_time_ms }}` | Tiempo de respuesta en milisegundos |
| `{{ error_message }}` | Mensaje de error (si lo hay) |
| `{{ status_code }}` | Código de estado HTTP (solo HTTP) |
| `{{ tags }}` | Lista de tags del monitor |
| `{{ checked_at }}` | Marca de tiempo del check |
| `{{ latency_threshold_ms }}` | Umbral de latencia (solo notificación de latencia) |
| `{{ latency_exceeded_by_ms }}` | Exceso sobre el umbral (solo latencia) |
| `{{ days_left }}` | Días hasta expiración (solo expiración) |
| `{{ expiry_threshold_days }}` | Días umbral de expiración (solo expiración) |

### 13.2. Plantillas por defecto

Se configuran en **Ajustes → Plantillas**. Se usan cuando un monitor no tiene plantilla personalizada.

**DOWN (por defecto):**
```
🔴 {{ monitor_name }} — {{ target }}
Status: {{ status }}
Response: {{ response_time_ms }}ms
{{ error_message }}
```

**LATENCIA (por defecto):**
```
🟡 {{ monitor_name }} — {{ target }}
High latency: {{ response_time_ms }}ms (threshold: {{ latency_threshold_ms }}ms, exceeded by {{ latency_exceeded_by_ms }}ms)
```

**UP (por defecto):**
```
🟢 {{ monitor_name }} — {{ target }}
Recovered — Status: {{ status }}
Response: {{ response_time_ms }}ms
```

**EXPIRACIÓN (por defecto):**
```
🟡 {{ monitor_name }} — {{ target }}
Certificate expires in {{ days_left }} days (threshold: {{ expiry_threshold_days }} days)
```

### 13.3. Plantillas por monitor

Cada monitor puede tener sus propias plantillas, visibles en la pestaña **Plantillas** al editar un monitor. Si están vacías, se usan las plantillas por defecto globales.

---

## 14. Backup y recuperación

### 14.1. Backup desde la UI

1. Ve a **Ajustes → Backup**.
2. Haz clic en **Crear backup ahora**.
3. WatchBeat fuerza un checkpoint WAL y copia el archivo SQLite a `watchbeat.backup.db`.

### 14.2. Backup manual

```bash
# Desde el host (Docker)
docker exec watchbeat sqlite3 /app/data/watchbeat.db ".backup /app/data/backup-$(date +%Y%m%d).db"

# O copiando el archivo (parando primero el contenedor)
docker stop watchbeat
cp /var/lib/docker/volumes/watchbeat_watchbeat_data/_data/watchbeat.db /backup/
docker start watchbeat
```

### 14.3. Restaurar backup

```bash
docker stop watchbeat
cp /backup/watchbeat.db /var/lib/docker/volumes/watchbeat_watchbeat_data/_data/
docker start watchbeat
```

---

## 15. Exportación e importación de configuración

WatchBeat puede exportar e importar toda la configuración en formato JSON: monitores, notificadores, status pages y ajustes.

### 15.1. Exportar

Ve a **Ajustes → Exportar / Importar** y haz clic en **Exportar todo**. Descargarás un archivo JSON con toda la configuración.

### 15.2. Importar

1. En **Ajustes → Exportar / Importar**, haz clic en **Importar desde archivo**.
2. Selecciona un archivo JSON exportado previamente.
3. WatchBeat actualizará los elementos existentes y creará los nuevos.

### 15.3. Automatización con API

```bash
# Exportar
curl -H "Authorization: Bearer $TOKEN" \
  https://watchbeat.tudominio.com/api/export > watchbeat-export.json

# Importar
curl -X POST -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d @watchbeat-export.json \
  https://watchbeat.tudominio.com/api/import
```

> Para obtener el token JWT, abre las DevTools del navegador (F12), ve a Application → Session Storage → `watchbeat_token` tras iniciar sesión.

---

## 16. Mantenimiento

### 16.1. Retención de datos

WatchBeat limpia automáticamente los checks antiguos. La retención se configura en **Ajustes → Retención de datos**:

- **7 días**: mínimo espacio en disco
- **14 días**: defecto recomendado
- **15-30 días**: más histórico

El cleanup se ejecuta una vez al día (1 hora después del arranque).

### 16.2. Logs

```bash
# Ver logs
docker compose logs -f
docker compose logs -f --tail=100

# Filtrar por nivel
docker compose logs -f | grep "WARN"
docker compose logs -f | grep "ERROR"
```

Formato de logs configurable con `LOG_FORMAT`:
- `pretty` (defecto): legible para humanos, con colores
- `json`: estructurado, ideal para ingestión (Loki, Elastic, etc.)

### 16.3. Actualización

```bash
cd /opt/watchbeat
git pull
docker compose build --no-cache
docker compose up -d
```

Con `just`:
```bash
just upgrade   # incrementa patch, compila, construye imagen, pushea
```

### 16.4. Optimización SQLite

WatchBeat ya configura SQLite con modo WAL y foreign_keys. Si tienes muchos datos, puedes ejecutar VACUUM periódicamente:

```bash
docker exec watchbeat sqlite3 /app/data/watchbeat.db "VACUUM;"
```

---

## 17. Monitorización de WatchBeat

### 17.1. Endpoint /health

```bash
curl https://watchbeat.tudominio.com/health
# {"status":"ok"}
```

Usa este endpoint para tu sistema de monitorización (Uptime Kuma, Grafana, Prometheus, etc.)

### 17.2. Métricas Prometheus

```bash
curl https://watchbeat.tudominio.com/metrics
```

Expone:
```
# HELP watchbeat_checks Total checks
# TYPE watchbeat_checks counter
watchbeat_checks 1234

# HELP watchbeat_monitors_up Monitors up
# TYPE watchbeat_monitors_up gauge
watchbeat_monitors_up 8

# HELP watchbeat_monitors_down Monitors down
# TYPE watchbeat_monitors_down gauge
watchbeat_monitors_down 2
```

### 17.3. Integración con Grafana

Añade WatchBeat como target de Prometheus (o usa el endpoint `/metrics` directo con el plugin Prometheus de Grafana) y crea paneles con:

- `rate(watchbeat_checks[5m])` — tasa de checks
- `watchbeat_monitors_up` — monitores activos
- `watchbeat_monitors_down` — monitores caídos

---

## 18. Variables de entorno — referencia completa

| Variable | Obligatoria | Defecto | Descripción |
|---|---|---|---|
| `OIDC_ISSUER_URL` | ✅ | — | URL del issuer OIDC (ej: `https://auth.tudominio.com/application/o/watchbeat/`) |
| `OIDC_CLIENT_ID` | ✅ | — | Client ID registrado en el proveedor OIDC |
| `OIDC_CLIENT_SECRET` | ✅ | — | Client Secret del proveedor OIDC |
| `OIDC_REDIRECT_URL` | | `http://localhost:3055/auth/callback` | URL de callback. Debe coincidir EXACTAMENTE con el proveedor. |
| `PORT` | | `3055` | Puerto de escucha del servidor |
| `HOST` | | `0.0.0.0` | Host de escucha (`0.0.0.0` para todas las interfaces) |
| `DATA_DIR` | | `./data` | Directorio para datos persistentes |
| `DATABASE_URL` | | `./data/watchbeat.db` | Ruta completa al archivo SQLite |
| `TIMEZONE` | | `Europe/Madrid` | Zona horaria. Ver [lista de timezones](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones) |
| `RUST_LOG` | | `info` | Nivel de log: `trace`, `debug`, `info`, `warn`, `error` |
| `LOG_FORMAT` | | `pretty` | Formato: `pretty` (legible) o `json` (estructurado) |
| `WATCHBEAT_RETENTION_DAYS` | | `30` | Días de retención (se sobreescribe desde la UI) |

---

## 19. Solución de problemas

### 19.1. Error: "OIDC env var X is required"

Falta una variable de entorno obligatoria. Revisa tu `.env`.

```bash
# Verificar que las variables están cargadas
docker compose config | grep OIDC
```

### 19.2. Error: "OIDC discovery failed"

WatchBeat no puede contactar con tu proveedor OIDC. Causas comunes:

- **URL incorrecta**: verifica `OIDC_ISSUER_URL`. Debe terminar en `/.well-known/openid-configuration`
- **Firewall**: el VPS debe poder llegar al proveedor OIDC
- **DNS**: los nombres deben resolverse
- **Certificado TLS**: si el proveedor tiene un certificado auto-firmado, no funcionará

```bash
# Probar desde el contenedor
docker exec watchbeat wget -O- https://auth.tudominio.com/.well-known/openid-configuration
```

### 19.3. Error: "JWKS fetch failed"

WatchBeat no pudo obtener las claves públicas del proveedor OIDC para validar tokens. Suele ser el mismo problema que 19.2.

### 19.4. Error en login: "redirect_uri mismatch"

La `OIDC_REDIRECT_URL` no coincide exactamente con la registrada en el proveedor. Revisa:

- **http vs https** — deben coincidir
- **Barra al final** — no debe haber diferencia
- **Puerto** — si el proveedor espera 3055 y tu reverse proxy sirve en 443
- **Path** — siempre debe ser `/auth/callback`

### 19.5. El healthcheck no funciona

```bash
# Probar manualmente
docker exec watchbeat wget --spider http://localhost:3055/health

# Ver logs de Docker
docker compose logs watchbeat
```

### 19.6. El frontend no carga (blanco)

- **Proxy reverso**: verifica que el path `/` está correctamente redirigido
- **SPA embebida**: si compilaste manualmente, asegúrate de que `frontend/dist` existe antes de compilar el backend
- **Logs del backend**: busca errores de embed o de rutas

### 19.7. Los checks no se ejecutan

```bash
# Verificar scheduler
docker compose logs watchbeat | grep Scheduler

# Verificar estado del scheduler desde API
curl -H "Authorization: Bearer $TOKEN" https://watchbeat.tudominio.com/api/me
```

Causas:
- El scheduler espera 10 segundos al arrancar antes de la primera ejecución
- Los monitores deben tener `enabled: true`
- Comprueba que los monitores tienen un checker válido

### 19.8. No llegan notificaciones

1. Prueba el notificador desde **Ajustes → Notificadores** con el botón **Probar**
2. Verifica que el notificador está **habilitado**
3. Comprueba que el monitor tiene el notificador **asignado**
4. Revisa los logs del backend: busca mensajes del scheduler sobre notificaciones

### 19.9. Error "Address already in use"

El puerto 3055 ya está ocupado. Cambia `PORT` en `.env` y en el `compose.yml`.

### 19.10. Error de permisos en SQLite

```bash
# El usuario del contenedor (app, uid 1000) debe poder escribir en /app/data
docker compose exec watchbeat ls -la /app/data
docker compose exec watchbeat touch /app/data/test
```

---

## Consejos finales

- **Empieza con pocos monitores** (2-3) para verificar que todo funciona antes de añadir más.
- **Configura retención a 7 días** si tienes muchos monitores y poco disco.
- **Usa HEAD en HTTP** — es más rápido y consume menos ancho de banda que GET.
- **Añade confirmaciones** (2-3) a monitores críticos para evitar falsos positivos.
- **Prueba los notificadores** antes de asignarlos a monitores.
- **Revisa los logs** semanalmente para detectar problemas antes de que sean críticos.
- **Manten actualizado** — WatchBeat recibe mejoras y correcciones con frecuencia.

---

> ¿Problemas o dudas? [Abre un issue](https://github.com/atareao/watchbeat/issues) en GitHub.