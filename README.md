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
| **Export/Import** | Exporta e importa toda la configuración (monitores, notificadores, status pages, ajustes) en JSON |
| **Retención** | Limpieza automática de checks antiguos (configurable, defecto 30 días) |
| **SPA embebida** | Frontend compilado dentro del binario — un solo proceso, cero dependencias runtime |
| **Docker** | Build multi-stage, healthcheck, compose listo para producción |

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

## Checkers — Tipos de monitor

| Tipo | ¿Qué comprueba? | Target típico | Ejemplo |
|---|---|---|---|
| **HTTP(S)** | Petición HTTP y verifica código de estado (2xx/3xx o el esperado) y contenido del body. Soporta GET, HEAD, POST, `expected_status`, `expected_body` y regex. | URL completa | `https://atareao.es` |
| **TLS/SSL** | Handshake TLS y comprueba la fecha de expiración del certificado. Alerta cuando quedan pocos días. | host (sin `https://`) | `atareao.es` |
| **TCP** | Conexión TCP al puerto — verifica que el servicio escucha. | host:puerto | `atareao.es:443` |
| **Ping** | Ejecuta `ping -c 1` — verifica respuesta ICMP. | IP o dominio | `8.8.8.8` |
| **Heartbeat** | Espera un pulso HTTP (`POST /api/heartbeat/{token}`). Evalúa si el servicio sigue vivo según `grace_seconds`. | — | Un token UUID por monitor |

---

## Notificadores — Canales de alerta

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

Los notificadores se configuran desde la interfaz web (Settings → Notificadores). Cada monitor puede tener múltiples notificadores asociados (relación many-to-many).

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

## Requisitos

- **OIDC obligatorio** — Necesitas un proveedor OIDC. Recomendado: [Authentik](https://goauthentik.io/) (self-hosted). Alternativas: Keycloak, Authelia, Google Workspace, Azure AD, Okta.
- **Docker** o **Podman** (para producción)
- Rust toolchain + Node.js (solo para desarrollo)

---

## ⚡ Desarrollo rápido

```bash
# Backend
cd backend
cp ../watchbeat.env.example .env
# Rellena OIDC_* en .env
cargo run

# Frontend (otra terminal)
cd frontend
npm install
npm run dev
```

Frontend en `http://localhost:3050`, backend en `http://localhost:3055`.

---

## 🐳 Docker / Podman

Con Docker Compose:

```bash
cp watchbeat.env.example .env
# Edita .env con tus valores OIDC
docker compose up -d
```

Con Podman directamente:

```bash
just build
just push     # si tienes configurado un registry
```

Manual:

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

### compose.yml — configuración

El `compose.yml` incluye:

- Volumen nombrado `watchbeat_data` para persistencia SQLite
- Healthcheck con `wget --spider /health`
- Límites de memoria: 256M máximo, 64M reservados
- Logging rotatorio: 3 archivos de 10MB
- Política de reinicio: `unless-stopped`
- Carga automática de `.env`

---

## ⚙️ Variables de entorno

| Variable | Obligatoria | Defecto | Descripción |
|---|---|---|---|
| `OIDC_ISSUER_URL` | ✅ | — | URL del issuer OIDC |
| `OIDC_CLIENT_ID` | ✅ | — | Client ID en el proveedor OIDC |
| `OIDC_CLIENT_SECRET` | ✅ | — | Client Secret |
| `OIDC_REDIRECT_URL` | | `http://localhost:3055/auth/callback` | URL de callback (debe coincidir con el proveedor) |
| `PORT` | | `3055` | Puerto de escucha |
| `HOST` | | `0.0.0.0` | Host de escucha |
| `DATA_DIR` | | `./data` | Directorio de datos |
| `DATABASE_URL` | | `./data/watchbeat.db` | Ruta al archivo SQLite |
| `TIMEZONE` | | `Europe/Madrid` | Zona horaria |
| `RUST_LOG` | | `info` | Nivel de log |
| `LOG_FORMAT` | | `pretty` | Formato: `pretty` o `json` |
| `WATCHBEAT_RETENTION_DAYS` | | `30` | Días de retención de checks (configurable desde UI) |

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