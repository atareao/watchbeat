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
│   │   ├── main.rs           # Entrypoint + SchedulerManager::spawn()
│   │   ├── lib.rs            # Re-exporta todos los módulos
│   │   ├── config.rs         # Env vars → Config (OIDC obligatorio)
│   │   ├── db.rs             # SQLite + migraciones + CRUD (sqlx)
│   │   ├── auth.rs           # OIDC discovery + JWKS + JWT validation
│   │   ├── models.rs         # Monitor, CheckResult, Notifier, etc.
│   │   ├── embed.rs          # SPA embebida (include_dir!)
│   │   ├── scheduler.rs      # SchedulerManager + per-monitor tokio timers
│   │   ├── checker/          # http, tcp, ping, tls
│   │   ├── notifier/         # 8 tipos: telegram, matrix, ntfy, webhook, slack, discord, email, gotify
│   │   └── routes/           # 11 módulos de rutas (sin metrics.rs)
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
| perf | ⚡ |
| docs | 📝 |
| test | ✅ |
| style | 🎨 |
| chore | 🔧 |
| rename | 🏷️ |

### Versionado
- `vampus` tool gestiona la versión en `Cargo.toml`
- `just upgrade` incrementa patch, hace `cargo update`, commitea y tagea

## Detalles técnicos que un agente puede pasar por alto

### Backend — Scheduler

- **SchedulerManager** en `scheduler.rs` — reemplaza el antiguo bucle polling global. Cada monitor tiene su propio `tokio::time::interval` con `MissedTickBehavior::Skip` (evita bucles si un check se retrasa).
- **Comandos**: `SchedulerCommand` enum (`Spawn`, `Update`, `Remove`, `ReloadNotifiers`) enviados por canal `tokio::sync::mpsc` desde las rutas CRUD.
- **Panic recovery**: `monitor_task` usa `AssertUnwindSafe` + `.catch_unwind()` para reiniciar el task del monitor tras 30s si panic.
- **Caches en memoria por monitor**: checker (`Box<dyn Checker>`), `was_up: bool`, `notifier_ids: Vec<String>` — **0 queries SQLite por check**.
- **Writes reducidos**: solo se inserta en `checks` cuando cambia el estado o cada 10º check (muestreo de latencia). Monitores estables → ~0 writes/min.
- **SSE event**: solo se aloca el JSON si `event_tx.receiver_count() > 0` (sin frontend → 0 allocs).
- **`run_monitor_check()`** acepta `checker: Option<&dyn Checker>`, `was_up: bool`, `notifier_ids: &[String]`, `check_count: u64` y devuelve `bool` (nuevo `is_up`).
- **`last_check_at`**: `AtomicI64` (timestamp unix) en vez de `RwLock<Option<String>>`.

### Backend — SQLite

- **WAL mode** con `synchronous=NORMAL` (crash-safe con WAL, ~50x más rápido que FULL).
- **Pragmas**: `journal_size_limit=65536`, `cache_size=-8000` (8MB), `busy_timeout=5000`, `temp_store=memory`.
- **Índice cubriente**: `idx_checks_uptime(monitor_id, checked_at, status)` para queries de uptime (index-only scan).
- **`max_connections=4`**, foreign_keys ON.
- **Prometheus eliminado**: no hay `/metrics` endpoint, no hay dependencia `prometheus-client`, no hay timer cada 30s.

### Backend — HTTP Client

- **`reqwest::Client` global** con `OnceLock` en `checker/mod.rs` — se crea una vez al arrancar, reutilizado en todos los checks HTTP. Pool de conexiones, TLS, DNS cacheados.
- Timeout por request (`.timeout(timeout)`), no por cliente.

### Backend — General

- **OIDC es OBLIGATORIO** — sin proveedor OIDC el binario no arranca (panic en `env_required`).
- **`[[bin]]` + `[lib]`** — los tests de integración importan `watchbeat::db::Database` desde el lib crate.
- **Backend embebe el frontend** — `include_dir!` en `embed.rs` compila `frontend/dist` dentro del binario. Para desarrollo local se usa el proxy de Vite, no el binario embebido.
- **Checker trait** — `#[async_trait]` con `fn check(&self, monitor: &Monitor) -> CheckOutcome`.
- **Notifier dispatch** — en `scheduler.rs` `run_monitor_check()`, dispatch manual por tipo (no usa `NotifierTrait`).
- **Confirmación de caída** — `confirmations_required` + `failed_checks` evitan falsos positivos.
- **Retención configurable** — `settings.retention_days`, default 30, cleanup cada 24h.
- **Rutas públicas** que saltan el middleware JWT: `/auth/`, `/health`, `/`, `/api/heartbeat/`, `/api/events`.
- **Nullable fields en update**: todos los campos `Option` (`latency_threshold_ms`, `notifier_id`, `message_template_*`, `grace_seconds`) se asignan directamente desde el request (`req.campo`), NO con `.or(existing.campo)` — esto permite que `null` limpie el valor en DB.

### Frontend — SSE

- **Hook `useSse`** en `hooks/useSse.ts` — conecta `EventSource` a `/api/events?token=<jwt>`, auto-reconnect 5s, cleanup en unmount.
- **Eventos**: tipo `CheckEvent` con `{ type: "check", monitor_id, monitor_name, status, response_time_ms, error_message, checked_at }`.
- **Dashboard**: `load()` con spinner para carga inicial/filtros; `refresh()` silencioso (sin spinner) para SSE y fallback poll 60s.
- **MonitorDetail**: `latestCheck` independiente de la tabla filtrada (que por defecto muestra solo errores). Se actualiza inmediatamente desde el evento SSE + re-fetch de monitor/buckets/checks.
- **Auth**: el token se pasa como query param `?token=` porque `EventSource` no soporta headers personalizados.

### Frontend — General

- **react-router v8** — imports desde `"react-router"`, NO `"react-router-dom"`.
- **Vite proxy** — `/api`, `/auth`, `/health` → `localhost:3055` en dev.
- **Auth** — JWT en `sessionStorage` + `localStorage` (clave `watchbeat_token`).
- **Dark mode** — `localStorage` clave `watchbeat-theme`, `ConfigProvider` con `darkAlgorithm`.
- **Sin tests de frontend** — no hay test runner configurado.
- **Lazy loading** — todas las páginas con `React.lazy()` + `Suspense`.
- **Formularios**: campos opcionales (`latency_threshold_ms`, `notifier_id`) se inicializan con `?? undefined` (no `null`) para que Ant Design los muestre vacíos. En submit, `undefined` se convierte a `null` para enviar a la API.

### Docker

- **Multi-stage**: `node:23-alpine` (frontend) → `rust:alpine3.23` (backend) → `alpine:3.23` (runtime).
- **El backend necesita el frontend compilado** — `COPY --from=frontend-builder` antes de compilar Rust.
- **Healthcheck**: `wget --spider http://localhost:3055/health`.
- **Puerto**: 3055 (configurable via `WATCHBEAT_PORT`).
- **compose.yml** es el canónico; `docker-compose.yml` es alias legacy.

### Testing

- Tests unitarios inline (`#[cfg(test)] mod tests`) en `checker/mod.rs`, `config.rs`.
- Tests de integración en `backend/tests/db_integration.rs` (usa `tempfile`).
- No hay tests de frontend.
- Para tests de integración: `cargo test --test db_integration`.

### Archivos que no deberían editarse

- `docker-compose.yml` — alias legacy, editar `compose.yml`.
- `GIT_FLOW.md` — duplicado de info en README, mantener sincronizado.

## Referencias

- `PLAN.md` — roadmap de features pendientes (metric consolidation, status pages, export, backup, swagger).
- `GIT_FLOW.md` — convenciones de ramas y commits.
- `watchbeat.env.example` — todas las variables de entorno documentadas.