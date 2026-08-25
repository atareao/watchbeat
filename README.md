# 🕵️ WatchBeat

Monitor de uptime auto-hosteado con backend Rust + Axum y frontend React + Ant Design.

## Stack

- **Backend:** Rust, Axum, SQLite (sqlx 0.9), OIDC (autenticación), reqwest 0.13 (HTTP checks)
- **Frontend:** React 19, TypeScript 7, Vite 8, Ant Design 6
- **Infra:** Docker multi-stage, Git Flow, conventional commits

## Arquitectura

```
watchbeat/
├── backend/              # Rust + Axum + SQLite
│   └── src/
│       ├── main.rs             # Entrypoint + scheduler loop
│       ├── config.rs           # Env vars → Config (OIDC obligatorio)
│       ├── db.rs               # SQLite + migraciones + CRUD
│       ├── auth.rs             # OIDC discovery + JWKS + JWT validation
│       ├── embed.rs            # SPA embebida (include_dir!)
│       ├── models.rs           # Monitor, CheckResult, Notifier, Timeline
│       ├── checker/            # HTTP, TCP, Ping checkers
│       ├── notifier/           # Telegram notifier
│       └── routes/             # API endpoints
├── frontend/             # React 19 + Vite + Ant Design
│   └── src/
│       ├── pages/              # Dashboard, Monitors, MonitorDetail, Notifiers
│       ├── components/         # AppLayout, MonitorCard
│       ├── api/http.ts         # Fetcher genérico con auth JWT
│       └── store/auth.ts       # Auth store
└── Dockerfile            # Multi-stage build (Rust + Node)
```

## Checkers

| Tipo | Cómo funciona |
|------|--------------|
| **HTTP(S)** | reqwest GET/HEAD con timeout. Status 2xx/3xx = UP. Opción expected_status. |
| **TCP** | tokio TcpStream connect con timeout. |
| **Ping** | ping -c 1 con timeout. Parsea latencia de la salida. |

## API

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/auth/login` | Login OIDC |
| `GET` | `/auth/callback` | Callback OIDC (PKCE) |
| `GET` | `/api/me` | Usuario actual |
| `GET/POST` | `/api/monitors` | Listar / crear monitores |
| `PUT/DELETE/PATCH` | `/api/monitors/{id}` | Actualizar / eliminar / toggle |
| `POST` | `/api/monitors/{id}/check` | Ejecutar check manual |
| `GET` | `/api/monitors/{id}/checks` | Histórico checks |
| `GET` | `/api/monitors/{id}/timeline` | Timeline para gráfica |
| `GET` | `/api/checks/recent` | Último check de cada monitor |
| `GET/POST` | `/api/notifiers` | Listar / crear notificadores |
| `PUT/DELETE` | `/api/notifiers/{id}` | Actualizar / eliminar |
| `POST` | `/api/notifiers/{id}/test` | Enviar notificación de prueba |
| `GET` | `/api/status` | Dashboard (stats + summaries) |

## Requisitos

- **OIDC obligatorio** — Necesitas un proveedor OIDC (Authelia, Authentik, Keycloak, etc.)
- Docker (para producción) o Rust toolchain + Node (para desarrollo)

## Desarrollo rápido

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

## Docker

```bash
docker build -t watchbeat .
docker run -p 3055:3055 \
  -v ./data:/app/data \
  -e OIDC_ISSUER_URL=https://auth.tuservidor.com \
  -e OIDC_CLIENT_ID=watchbeat \
  -e OIDC_CLIENT_SECRET=secreto \
  watchbeat
```

## Git Flow

Este proyecto sigue **Git Flow** como modelo de ramas.

### Convenciones

| Rama base | Propósito |
|-----------|----------|
| `main` | Producción — solo recibe merges desde `release/` o `hotfix/` |
| `development` | Integración — rama base para `feature/` y `release/` |

### Prefijos

| Prefijo | Uso | Origen | Destino |
|---------|-----|--------|--------|
| `feature/` | Nuevas funcionalidades | `development` | `development` |
| `release/` | Preparación de versión | `development` | `main` + `development` |
| `hotfix/` | Correcciones urgentes en producción | `main` | `main` + `development` |
| `support/` | Ramas de soporte a largo plazo | `main` | — |

### Flujo diario

```bash
# 1. Empezar una feature
git flow feature start <nombre>

# 2. Trabajar, commitear, pushear
git add . && git commit -m "✨ feat: ..."
git flow feature publish <nombre>

# 3. Terminar la feature (merge a development)
git flow feature finish <nombre>

# 4. Preparar una release
git flow release start <versión>
# Ajustar versiones, CHANGELOG, etc.
git flow release finish <versión>

# 5. Hotfix urgente
git flow hotfix start <versión>
git flow hotfix finish <versión>
```

### Commits

Usamos [Conventional Commits](https://www.conventionalcommits.org/):

| Tipo | Significado |
|------|-------------|
| `✨ feat:` | Nueva funcionalidad |
| `🐛 fix:` | Corrección de bug |
| `🔒 fix:` | Corrección de seguridad |
| `♻️ refactor:` | Refactorización |
| `📝 docs:` | Documentación |
| `✅ test:` | Tests |
| `🎨 style:` | Formato, estilo |
| `🔧 chore:` | Mantenimiento, CI, build |
| `🏷️ rename:` | Renombrados |

## Licencia

MIT