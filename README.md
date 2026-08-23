# 🕵️ WatchBeat

Monitor de uptime auto-hosteado con backend Rust + Axum y frontend React + Ant Design.

## Stack

- **Backend:** Rust, Axum, SQLite (rusqlite), OIDC (autenticación), reqwest (HTTP checks)
- **Frontend:** React 19, TypeScript, Vite, Ant Design 5
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

## Licencia

MIT