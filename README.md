# 🕵️ WatchBeat

> **Monitor de uptime auto-hosteado** — backend Rust + Axum + SQLite, frontend React 19 + TypeScript 7 + Ant Design 6.

WatchBeat te permite monitorizar tus servicios desde tu propio servidor. Sin dependencias externas, sin SaaS, sin límites artificiales. Conéctalo a tu proveedor OIDC, define tus monitores, y recibe alertas cuando algo falle.

Diseñado para ser **eficiente**: con 100 monitores estables, el backend escribe ~0 veces por minuto a SQLite y consume ~0% de CPU cuando no hay cambios de estado.

---

## ✨ Características

| Característica | Descripción |
|---|---|
| **Checkers** | HTTP(S), TCP, Ping, TLS/SSL |
| **Heartbeats** | Monitores que esperan un pulso periódico desde un servicio (push) |
| **Notificaciones** | 8 canales: Telegram, Matrix, ntfy, Webhook, Slack, Discord, Email (SMTP), Gotify |
| **Autenticación** | OIDC obligatorio (Authentik, Keycloak, Authelia, Google, Azure AD, etc.) |
| **Alertas inteligentes** | Confirmaciones configurables antes de marcar DOWN, umbrales de latencia |
| **Plantillas** | Mensajes personalizables con Jinja2 para DOWN, UP, latencia y expiración de certificado |
| **Certificados TLS** | Monitorización de expiración con alertas anticipadas |
| **Status Pages** | Páginas públicas de estado eligiendo qué monitores mostrar |
| **Tiempo real** | Eventos SSE en vivo sobre el estado de los checks (sin polling) |
| **Gráficas** | Timeline de uptime por monitor con buckets (6h, 12h, 24h, 7d, 15d, 30d, 3m, 6m, 1a) |
| **Dashboard** | Estadísticas globales, búsqueda, filtros por tipo/estado, paginación |
| **Dark mode** | Tema oscuro/claro con persistencia en localStorage |
| **Backup** | Copia de seguridad de la BD SQLite desde la interfaz |
| **Export/Import** | Exporta e importa toda la configuración en JSON |
| **Retención** | Limpieza automática de checks antiguos (configurable, defecto 30 días) |
| **SPA embebida** | Frontend compilado dentro del binario — un solo proceso, cero dependencias runtime |
| **Docker** | Build multi-stage, healthcheck, compose listo para producción |

---

## ⚡ Quick Start

```bash
# 1. Clona el repositorio
git clone https://github.com/atareao/watchbeat.git
cd watchbeat

# 2. Configura variables de entorno
cp watchbeat.env.example .env
# Edita .env con tu proveedor OIDC (ver sección Configuración)

# 3. Arranca con Docker Compose
docker compose up -d

# 4. Abre http://localhost:3055 y login con OIDC
```

---

## 📦 Instalación

### Requisitos

- **OIDC obligatorio** — Necesitas un proveedor OIDC. Recomendado: [Authentik](https://goauthentik.io/) (self-hosted). Alternativas: Keycloak, Authelia, Google Workspace, Azure AD, Okta.
- **Docker** o **Podman** (para producción)
- Rust toolchain + Node.js (solo para desarrollo)

### Con Docker Compose (recomendado)

```bash
cp watchbeat.env.example .env
# Edita .env con tus valores (ver sección Configuración)
docker compose up -d
```

Esto arranca WatchBeat en `http://localhost:3055` con:

- Volumen `watchbeat_data` para persistencia SQLite
- Healthcheck automático
- Límite de memoria: 256M máximo, 64M reservados
- Logging rotatorio: 3 archivos de 10MB
- Reinicio automático (`unless-stopped`)

### Con Podman

```bash
just build
just push     # si tienes registry configurado
```

### Manual (sin Docker)

```bash
docker build -t watchbeat .
docker run -p 3055:3055 \
  -v watchbeat_data:/app/data \
  -e OIDC_ISSUER_URL=https://auth.tudominio.com \
  -e OIDC_CLIENT_ID=watchbeat \
  -e OIDC_CLIENT_SECRET=secreto \
  -e OIDC_REDIRECT_URL=https://watchbeat.tudominio.com/auth/callback \
  watchbeat
```

### Desarrollo local

```bash
# Terminal 1 — Backend
cd backend
cp ../watchbeat.env.example .env
# Rellena OIDC_* en .env
cargo run

# Terminal 2 — Frontend
cd frontend
npm install
npm run dev
```

Frontend en `http://localhost:3050`, backend en `http://localhost:3055`.

---

## ⚙️ Configuración

### Variables de entorno

| Variable | Obligatoria | Defecto | Descripción |
|---|---|---|---|
| `OIDC_ISSUER_URL` | ✅ | — | URL del issuer OIDC (ej: `https://auth.tudominio.com/application/o/watchbeat`) |
| `OIDC_CLIENT_ID` | ✅ | — | Client ID en el proveedor OIDC |
| `OIDC_CLIENT_SECRET` | ✅ | — | Client Secret |
| `OIDC_REDIRECT_URL` | | `http://localhost:3055/auth/callback` | URL de callback (debe coincidir con el proveedor) |
| `PORT` | | `3055` | Puerto de escucha |
| `HOST` | | `0.0.0.0` | Host de escucha |
| `DATA_DIR` | | `./data` | Directorio de datos |
| `DATABASE_URL` | | `./data/watchbeat.db` | Ruta al archivo SQLite |
| `TIMEZONE` | | `Europe/Madrid` | Zona horaria |
| `RUST_LOG` | | `info` | Nivel de log (`debug`, `info`, `warn`, `error`) |
| `LOG_FORMAT` | | `pretty` | Formato: `pretty` o `json` |
| `WATCHBEAT_RETENTION_DAYS` | | `30` | Días de retención de checks (configurable desde UI) |

### Configurar OIDC con Authentik (ejemplo)

1. En Authentik, ve a **Applications → Providers → Create Provider**:
   - Tipo: **OAuth2/OpenID Provider**
   - Client ID: `watchbeat`
   - Client Secret: genera uno
   - Redirect URIs: `https://watchbeat.tudominio.com/auth/callback`
   - Scopes: `openid email profile`

2. Crea una **Application**:
   - Slug: `watchbeat`
   - Provider: el que acabas de crear

3. En el `.env` de WatchBeat:
```bash
OIDC_ISSUER_URL=https://auth.tudominio.com/application/o/watchbeat
OIDC_CLIENT_ID=watchbeat
OIDC_CLIENT_SECRET=lo-que-generaste
OIDC_REDIRECT_URL=https://watchbeat.tudominio.com/auth/callback
```

### Configurar OIDC con Authelia (ejemplo)

En `configuration.yml` de Authelia:
```yaml
identity_providers:
  oidc:
    clients:
      - client_id: watchbeat
        client_secret: $pbkdf2-sha512$...  # hasheado con authelia hash-password
        redirect_uris:
          - https://watchbeat.tudominio.com/auth/callback
        scopes:
          - openid
          - email
          - profile
        grant_types:
          - authorization_code
        response_types:
          - code
```

---

## 🚀 Uso

### Primer login

1. Abre `http://localhost:3055` (o tu dominio)
2. Serás redirigido a tu proveedor OIDC para login
3. Tras autenticarte, vuelves al Dashboard de WatchBeat

### Crear un monitor HTTP

1. En el Dashboard, haz clic en **"Añadir monitor"**
2. Rellena:
   - **Nombre**: `Mi web`
   - **Tipo**: `HTTP`
   - **Target**: `https://ejemplo.com`
   - **Intervalo**: `5 minutos`
   - **Timeout**: `30 segundos`
3. Opcionalmente configura:
   - **Método HTTP**: GET, HEAD o POST
   - **Código esperado**: 200 (defecto), o 201, 301, etc.
   - **Body esperado**: texto o regex que debe contener la respuesta
4. Guarda

El monitor empezará a ejecutar checks automáticamente. Verás el estado en el Dashboard en tiempo real vía SSE.

### Crear un heartbeat

Los heartbeats son monitores que esperan un pulso HTTP desde tu servicio:

1. Crea un monitor de tipo **Heartbeat**
2. WatchBeat genera un token UUID único
3. Tu servicio envía pulsos periódicos a:
   ```
   POST /api/heartbeat/{token}
   ```
4. Si no recibe un pulso dentro del `grace_seconds`, marca el monitor como DOWN

### Configurar notificaciones

1. Ve a **Settings → Notificadores**
2. Añade un notificador (ej: Telegram):
   - **Tipo**: Telegram
   - **Nombre**: `Mi bot`
   - **Bot Token**: el token de tu bot de Telegram
   - **Chat ID**: el ID del chat donde recibirás alertas
3. Guarda
4. Asocia el notificador a uno o varios monitores desde el formulario de edición del monitor

### Tipos de monitor disponibles

| Tipo | ¿Qué comprueba? | Target típico | Ejemplo |
|---|---|---|---|
| **HTTP(S)** | Petición HTTP y verifica código de estado (2xx/3xx o el esperado) y contenido del body. Soporta GET, HEAD, POST, `expected_status`, `expected_body` y regex. | URL completa | `https://atareao.es` |
| **TLS/SSL** | Handshake TLS y comprueba la fecha de expiración del certificado. Alerta cuando quedan pocos días. | host (sin `https://`) | `atareao.es` |
| **TCP** | Conexión TCP al puerto — verifica que el servicio escucha. | host:puerto | `atareao.es:443` |
| **Ping** | Ejecuta `ping -c 1` — verifica respuesta ICMP. | IP o dominio | `8.8.8.8` |
| **Heartbeat** | Espera un pulso HTTP (`POST /api/heartbeat/{token}`). Evalúa si el servicio sigue vivo según `grace_seconds`. | — | Un token UUID por monitor |

### Canales de notificación

| Tipo | Canal | Campos necesarios |
|---|---|---|
| **Telegram** | Bot de Telegram | `bot_token`, `chat_id` |
| **Matrix** | Sala de Matrix | `homeserver_url`, `access_token`, `room_id` |
| **ntfy** | ntfy.sh o self-hosted | `topic`, `server_url` (opcional), `token` (opcional) |
| **Webhook** | URL arbitraria | `url`, `method` (opcional), `headers` (opcional) |
| **Slack** | Slack Webhook | `webhook_url` |
| **Discord** | Discord Webhook | `webhook_url` |
| **Email** | SMTP | `smtp_host`, `smtp_port`, `username`, `password`, `from`, `to` |
| **Gotify** | Gotify self-hosted | `server_url`, `app_token`, `priority` (opcional) |

Cada monitor puede tener múltiples notificadores asociados (relación many-to-many).

### Plantillas de mensaje

Los mensajes de notificación son personalizables con Jinja2. Variables disponibles:

| Variable | Descripción |
|---|---|
| `{{ monitor_name }}` | Nombre del monitor |
| `{{ monitor_type }}` | Tipo de monitor (http, tcp, ping, etc.) |
| `{{ target }}` | Target del monitor |
| `{{ status }}` | Estado actual (up, down) |
| `{{ response_time_ms }}` | Tiempo de respuesta en ms |
| `{{ error_message }}` | Mensaje de error si lo hay |
| `{{ checked_at }}` | Fecha/hora del check |
| `{{ latency_threshold_ms }}` | Umbral de latencia configurado |

Ejemplo de plantilla DOWN:
```
⚠️ {{ monitor_name }} está DOWN
Target: {{ target }}
Error: {{ error_message }}
Último check: {{ checked_at }}
```

### Status pages

Puedes crear páginas públicas de estado para compartir con tus usuarios:

1. Ve a **Settings → Status Pages**
2. Crea una página con un slug (ej: `status`)
3. Selecciona qué monitores mostrar
4. Comparte `https://watchbeat.tudominio.com/status/mi-slug`

La status page es pública (no requiere autenticación).

---

## ⚡ Rendimiento

WatchBeat está optimizado para ser **ultra-ligero** en idle:

| Escenario | Antes (v0.11) | Ahora (v0.12) |
|---|---|---|
| **CPU idle** (sin frontend) | ~1% (polling cada 15s) | **~0%** (timers del SO) |
| **Queries SQLite por check** | 3 (get_latest + insert + notifier_ids) | **0** (caches en memoria) |
| **Writes a DB** (100 monitores estables) | ~20 INSERTs/min | **~0** (solo cambios de estado) |
| **reqwest::Client** | Creado en cada check (TLS+DNS+pool) | **Global** (OnceLock, reutilizado) |
| **SSE JSON allocation** | En cada check | Solo si hay clientes conectados |

### Cómo funciona

- **Scheduler**: Cada monitor tiene su propio `tokio::time::interval` con `MissedTickBehavior::Skip`. No hay polling global. El manager loop está bloqueado en `mpsc::Receiver::recv()` — 0 CPU.
- **Caches en memoria**: `checker` (Box), `was_up` (bool), `notifier_ids` (Vec) se crean una vez por monitor y se reutilizan en cada check.
- **Writes diferidos**: Solo se escribe a SQLite cuando el estado cambia (UP↔DOWN) o cada 10º check (muestreo de latencia). Monitores estables → 0 writes.
- **SQLite optimizado**: WAL mode con `synchronous=NORMAL` (~50x más rápido que FULL), índice cubriente `(monitor_id, checked_at, status)` para queries de uptime index-only.

---

## Stack

```
Backend:   Rust + Axum 0.8 + SQLite (sqlx 0.9, WAL) + reqwest 0.13
Frontend:  React 19 + TypeScript 7 + Vite 8 + Ant Design 6 + react-router 8
Infra:     Docker multi-stage, Podman, Just, Git Flow
```

---

## Arquitectura

```
watchbeat/
├── backend/                    # Rust ([[bin]] + [lib])
│   ├── src/
│   │   ├── main.rs             # Entrypoint + SchedulerManager::spawn()
│   │   ├── lib.rs              # Re-exporta todos los módulos
│   │   ├── config.rs           # Variables de entorno → Config
│   │   ├── db.rs               # SQLite + migraciones + CRUD (sqlx 0.9)
│   │   ├── auth.rs             # OIDC discovery + JWKS + JWT validation
│   │   ├── models.rs           # Monitor, CheckResult, Notifier, StatusPage, etc.
│   │   ├── embed.rs            # SPA embebida (include_dir!)
│   │   ├── template.rs         # Motor de plantillas Jinja2 + defaults
│   │   ├── scheduler.rs        # SchedulerManager + per-monitor tokio timers
│   │   ├── checker/            # HTTP, TCP, Ping, TLS checkers
│   │   ├── notifier/           # 8 tipos: telegram, matrix, ntfy, webhook,
│   │   │                       #   slack, discord, email, gotify
│   │   └── routes/             # 11 módulos de rutas
│   │       ├── auth_routes.rs  # Login/logout/callback OIDC
│   │       ├── monitors.rs     # CRUD + toggle + run check
│   │       ├── checks.rs       # Histórico + timeline buckets
│   │       ├── notifiers.rs    # CRUD + test
│   │       ├── status.rs       # Dashboard stats
│   │       ├── status_pages.rs # CRUD + página pública
│   │       ├── heartbeats.rs   # Endpoint público de pulso
│   │       ├── settings.rs     # Ajustes globales
│   │       ├── backup.rs       # Backup SQLite
│   │       ├── export_import.rs# Export/import JSON
│   │       └── exports.rs      # Export individual por monitor
│   └── tests/
│       └── db_integration.rs   # Tests de integración con SQLite tempfile
├── frontend/
│   └── src/
│       ├── main.tsx            # Entrypoint con ConfigProvider + BrowserRouter
│       ├── App.tsx             # Router + lazy-loaded pages
│       ├── api/http.ts         # Fetcher genérico con auth JWT + tipos
│       ├── store/auth.ts       # JWT en sessionStorage + localStorage
│       ├── hooks/              # useAuth, useSse, useTheme
│       ├── components/         # AppLayout (header + navegación), MonitorCard
│       └── pages/              # Dashboard, MonitorDetail, Settings, LoginPage
├── compose.yml                 # Docker Compose canónico
├── docker-compose.yml          # Alias legacy
├── Dockerfile                  # Multi-stage (node → rust → alpine)
├── .justfile                   # Task runner (check, lint, build, push, gitflow)
├── watchbeat.env.example       # Documentación de variables de entorno
├── GIT_FLOW.md                 # Convenciones Git Flow
├── AGENTS.md                   # Instrucciones para agentes IA
├── docs/                       # Documentación del proyecto
└── PLAN.md                     # Roadmap de features
```

---

## API

| Método | Ruta | Auth | Descripción |
|--------|------|------|-------------|
| `GET` | `/health` | No | Health check público |
| `GET` | `/auth/login` | No | Login OIDC |
| `GET` | `/auth/callback` | No | Callback OIDC (PKCE) |
| `GET` | `/auth/logout` | No | Cerrar sesión |
| `GET` | `/api/events` | Token (query) | SSE — eventos en vivo de checks |
| `GET` | `/status/{slug}` | No | Status page pública |
| `POST` | `/api/heartbeat/{token}` | No | Pulso de heartbeat |
| `GET` | `/api/me` | JWT | Usuario actual |
| `GET/POST` | `/api/monitors` | JWT | Listar / crear monitores (paginado, búsqueda, filtros) |
| `GET` | `/api/monitors/{id}` | JWT | Obtener un monitor |
| `PUT` | `/api/monitors/{id}` | JWT | Actualizar monitor |
| `DELETE` | `/api/monitors/{id}` | JWT | Eliminar monitor |
| `PATCH` | `/api/monitors/{id}` | JWT | Toggle enable/disable |
| `POST` | `/api/monitors/{id}/check` | JWT | Ejecutar check manual |
| `GET` | `/api/monitors/{id}/checks` | JWT | Histórico de checks |
| `GET` | `/api/monitors/{id}/timeline` | JWT | Timeline buckets |
| `GET` | `/api/checks/recent` | JWT | Último check de cada monitor |
| `GET/POST` | `/api/notifiers` | JWT | Listar / crear notificadores |
| `PUT` | `/api/notifiers/{id}` | JWT | Actualizar notificador |
| `DELETE` | `/api/notifiers/{id}` | JWT | Eliminar notificador |
| `POST` | `/api/notifiers/{id}/test` | JWT | Enviar notificación de prueba |
| `GET/POST` | `/api/status-pages` | JWT | Listar / crear status pages |
| `PUT` | `/api/status-pages/{id}` | JWT | Actualizar status page |
| `DELETE` | `/api/status-pages/{id}` | JWT | Eliminar status page |
| `GET` | `/api/monitors/{id}/export/{format}` | JWT | Exportar monitor individual |
| `POST` | `/api/backup` | JWT | Crear backup de la BD SQLite |
| `GET` | `/api/export` | JWT | Exportar toda la configuración (JSON) |
| `POST` | `/api/import` | JWT | Importar configuración (JSON) |
| `GET` | `/api/settings` | JWT | Obtener un ajuste por clave |
| `POST` | `/api/settings` | JWT | Guardar un ajuste |
| `GET` | `/api/status` | JWT | Dashboard stats globales |

> **Nota**: No hay endpoint `/metrics`. Prometheus fue eliminado en v0.12 porque nadie lo scrapeaba y consumía CPU cada 30s con queries a SQLite.

---

## 🧪 Tests

```bash
# Tests unitarios
cd backend && cargo test

# Tests de integración (requiere SQLite)
cd backend && cargo test --test db_integration

# Lint + format
cd backend && cargo clippy --all-targets --all-features
cd backend && cargo fmt
```

---

## 🔧 Justfile — comandos útiles

```bash
just check         # cargo fmt --check + cargo clippy (pre-commit)
just lint          # cargo clippy --all-targets --all-features
just fmt           # cargo fmt --check
just fmt-fix       # cargo fmt (repara)
just build         # podman build con tag de versión
just push          # podman push

# Git Flow (vía just)
just gf-feature name       # feature/name desde development
just gf-finish name        # merge --no-ff a development
just gf-release version    # release/version desde development
just gf-publish version    # merge a main + develop + tag
just gf-hotfix desc        # hotfix/desc desde main
just gf-hotfix-publish desc version
just gf-graph              # git log --oneline --graph --all -30

just upgrade       # incrementa patch, cargo update, tag, build, push
```

---

## Git Flow

Este proyecto sigue **Git Flow** como modelo de ramas. Ver `GIT_FLOW.md` para detalles.

| Rama | Propósito |
|---|---|
| `main` | Producción — solo merges desde `release/` o `hotfix/` |
| `development` | Integración — rama base para features |
| `feature/*` | Nuevas funcionalidades |
| `release/*` | Preparación de versión |
| `hotfix/*` | Correcciones urgentes en producción |

### Commits

Usamos [Conventional Commits](https://www.conventionalcommits.org/) con gitmoji:

| Tipo | Emoji |
|---|---|
| feat | ✨ |
| fix | 🐛 |
| fix (seguridad) | 🔒 |
| refactor | ♻️ |
| perf | ⚡ |
| docs | 📝 |
| test | ✅ |
| style | 🎨 |
| chore | 🔧 |
| rename | 🏷️ |

---

## Licencia

MIT — [atareao](https://github.com/atareao)