# AGENTS.md — WatchBeat

Instrucciones compactas para sesiones OpenCode en este repositorio.

## Stack

- **Backend**: Rust + Axum + SQLite (sqlx 0.9, WAL mode)
- **Frontend**: React 19 + TypeScript 7 + Vite 8 + Ant Design 6 + react-router v8
- **Infra**: Docker multi-stage, Podman, `just`, Git Flow

## Estructura

```
watchbeat/
├── backend/          # Rust ([[bin]] + [lib] en Cargo.toml)
│   ├── src/
│   │   ├── main.rs           # Entrypoint + scheduler loop
│   │   ├── lib.rs            # Re-exporta todos los módulos
│   │   ├── config.rs         # Env vars → Config (OIDC obligatorio)
│   │   ├── db.rs             # SQLite + migraciones + CRUD (sqlx)
│   │   ├── auth.rs           # OIDC discovery + JWKS + JWT validation
│   │   ├── models.rs         # Monitor, CheckResult, Notifier, etc.
│   │   ├── embed.rs          # SPA embebida (include_dir!)
│   │   ├── checker/          # http, tcp, ping, tls
│   │   ├── notifier/         # 8 tipos: telegram, matrix, ntfy, webhook, slack, discord, email, gotify
│   │   └── routes/           # 11 módulos de rutas
│   └── tests/
│       └── db_integration.rs # Tests de integración con SQLite (tempfile)
├── frontend/
│   └── src/
│       ├── App.tsx           # Router + lazy-loaded pages
│       ├── main.tsx          # Entrypoint con ConfigProvider (theme)
│       ├── api/http.ts       # Fetcher genérico con auth JWT
│       ├── store/auth.ts     # JWT en sessionStorage + localStorage
│       ├── hooks/            # useAuth, useSse, useTheme
│       ├── components/       # AppLayout (sidebar + header)
│       └── pages/            # 8 páginas lazy-loaded
├── compose.yml               # Docker Compose canónico
├── docker-compose.yml        # Legacy alias (apunta a compose.yml)
├── Dockerfile                # Multi-stage (frontend-builder → backend-builder → runtime)
├── .justfile                 # Task runner (check, lint, fmt, build, push, gitflow recipes)
├── GIT_FLOW.md               # Convenciones Git Flow
└── PLAN.md                   # Roadmap de features
```

## Comandos esenciales

```bash
# Pre-commit (siempre en este orden)
just check                    # cargo fmt --check + cargo clippy -- -D warnings

# Backend
cd backend && cargo build
cd backend && cargo test      # Tests unitarios + integración
cd backend && cargo test --test db_integration  # Solo integración
cd backend && cargo clippy --all-targets --all-features
cd backend && cargo fmt

# Frontend
cd frontend && npm run dev    # Dev server en :3050, proxy a backend :3055
cd frontend && npm run build  # tsc -b && vite build

# Docker
just build                    # podman build con tag de versión (vampus)
just push                     # podman push con authfile
docker compose -f compose.yml up -d

# Git Flow (vía just)
just gf-feature <name>        # feature/<name> desde development
just gf-finish <name>         # merge --no-ff a development
just gf-release <version>     # release/<version> desde development
just gf-publish <version>     # merge a main + develop + tag
just gf-hotfix <desc>         # hotfix/<desc> desde main
just gf-hotfix-publish <desc> <version>
just gf-graph                 # git log --oneline --graph --all -30
```

## Convenciones

### Git Flow
- `main` = producción, `development` = integración
- Prefijos: `feature/`, `release/`, `hotfix/`, `support/`
- Siempre `--no-ff` en merges
- Tags semánticas: `v0.1.0`, `v0.2.0`, etc.

### Commits (Conventional Commits + gitmoji)
| Tipo | Emoji |
|------|-------|
| feat | ✨ |
| fix | 🐛 |
| fix (seguridad) | 🔒 |
| refactor | ♻️ |
| docs | 📝 |
| test | ✅ |
| style | 🎨 |
| chore | 🔧 |
| rename | 🏷️ |

### Versionado
- `vampus` tool gestiona la versión en `Cargo.toml`
- `just upgrade` incrementa patch, hace `cargo update`, commitea y tagea

## Detalles técnicos que un agente puede pasar por alto

### Backend
- **OIDC es OBLIGATORIO** — sin proveedor OIDC el binario no arranca (panic en `env_required`)
- **`[[bin]]` + `[lib]`** — los tests de integración importan `watchbeat::db::Database` desde el lib crate
- **SQLite con WAL** — `journal_mode=Wal`, `max_connections=4`, foreign_keys ON
- **SSE en vivo** — `tokio::sync::broadcast` canal, endpoint `GET /api/events?token=`
- **Backend embebe el frontend** — `include_dir!` en `embed.rs` compila `frontend/dist` dentro del binario. Para desarrollo local se usa el proxy de Vite, no el binario embebido.
- **Checker trait** — `#[async_trait]` con `fn check(&self, monitor: &Monitor) -> CheckOutcome`
- **Notifier dispatch** — en `main.rs` scheduler, no usa el trait `NotifierTrait` de `notifier/mod.rs` (dispatch manual por tipo)
- **Confirmación de caída** — `confirmations_required` + `failed_checks` evitan falsos positivos
- **Retención configurable** — `settings.retention_days`, default 30, cleanup cada ciclo del scheduler

### Frontend
- **react-router v8** — imports desde `"react-router"`, NO `"react-router-dom"`
- **Vite proxy** — `/api`, `/auth`, `/health` → `localhost:3055` en dev
- **Auth** — JWT en `sessionStorage` + `localStorage` (clave `watchbeat_token`)
- **SSE** — `EventSource` con `?token=` en query param, fallback reconnect 5s
- **Dark mode** — `localStorage` clave `watchbeat-theme`, `ConfigProvider` con `darkAlgorithm`
- **Sin tests de frontend** — no hay test runner configurado
- **Lazy loading** — todas las páginas con `React.lazy()` + `Suspense`

### Docker
- **Multi-stage**: `node:23-alpine` (frontend) → `rust:alpine3.23` (backend) → `alpine:3.23` (runtime)
- **El backend necesita el frontend compilado** — `COPY --from=frontend-builder` antes de compilar Rust
- **Healthcheck**: `wget --spider http://localhost:3055/health`
- **Puerto**: 3055 (configurable via `WATCHBEAT_PORT`)
- **compose.yml** es el canónico; `docker-compose.yml` es alias legacy

### Testing
- Tests unitarios inline (`#[cfg(test)] mod tests`) en `checker/mod.rs`, `config.rs`
- Tests de integración en `backend/tests/db_integration.rs` (223 líneas, usa `tempfile`)
- No hay tests de frontend
- Para tests de integración: `cargo test --test db_integration`

### Archivos que no deberían editarse
- `docker-compose.yml` — alias legacy, editar `compose.yml`
- `GIT_FLOW.md` — duplicado de info en README, mantener sincronizado

## Referencias

- `PLAN.md` — roadmap de features pendientes (SSL, confirmación, body validation, multi-notifier, status pages, heartbeats, SSE, gráficas, dark mode, export, retención, backup, compose, swagger)
- `GIT_FLOW.md` — convenciones de ramas y commits
- `watchbeat.env.example` — todas las variables de entorno documentadas