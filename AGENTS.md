# AGENT DIRECTIVES: OPENSPEC (SDD) + TDD WORKFLOW

## ⚠️ REGLA DE ORO — LEER ANTES DE ACTUAR

**ANTES de escribir o editar CUALQUIER archivo de código fuente (Rust, TypeScript, JSX, CSS, etc.),
debes ejecutar `just check-spec` para confirmar que existe un change proposal aprobado.**

Si `just check-spec` falla:
1. DETENTE inmediatamente.
2. Informa al usuario que no hay un change proposal activo.
3. Pregunta si quiere crear uno con `openspec new change <feature>`.
4. NO escribas código hasta recibir aprobación explícita.

**SALTARSE ESTE PASO ES VIOLACIÓN DEL PROTOCOLO.**

---

## I. CORE PRINCIPLES & GOALS

- **Phase 0 — Legacy Support:** If modifying existing code without specs or tests, establish a baseline spec and characterization tests before introducing changes.
- **Phase 1 — SDD (OpenSpec):** No new code or tests may be written before a spec change proposal exists in `openspec/changes/<feature>/` and is approved by the user.
- **Phase 2 — TDD (Red-Green-Refactor):** Once the spec is approved, code MUST be developed strictly test-first using terminal commands.
- **Strict Verification:** Always run CLI test suites using terminal tools. Never assume code or tests pass/fail without CLI confirmation.

---

## II. EXECUTION WORKFLOW

### Phase 0: Legacy Code Preparation (Conditional)

*Execute this phase ONLY if modifying an existing module/file that lacks OpenSpec documentation or tests.*

1. **Characterization Spec (As-Is):**
   - Inspect the target file/module.
   - Generate a baseline spec in `openspec/specs/<module>/spec.md` reflecting current behavior.
2. **Characterization Tests:**
   - Write Rust (`#[test]`) or React/TS (`vitest` / `@testing-library/react`) tests matching current behavior.
   - Run tests via CLI (`cargo test` or `npx vitest run`) to confirm all pass in **GREEN**.

### Phase 1: SDD Protocol (OpenSpec)

When the user requests a new feature, bug fix, or refactor:

1. **Create the Change Proposal:**
   - Execute CLI command: `openspec new change <feature-name>`
2. **Draft Specifications:**
   - Populate `openspec/changes/<feature-name>/proposal.md` with intent, scope, and impact.
   - Create spec deltas in `openspec/changes/<feature-name>/specs/<module>/spec.md`.
   - Ensure the spec includes:
     - **Contracts:** Rust types/structs/enums, TypeScript interfaces/props, API endpoints, or function signatures.
     - **Scenarios (BDD style):** Detailed `Given / When / Then` clauses for happy path, error cases, and edge cases.
   - Populate `openspec/changes/<feature-name>/tasks.md` with the TDD task checklist.
3. **STOP & WAIT FOR APPROVAL:**
   - Present the created specification to the user.
   - **DO NOT** write application code or new tests until the user explicitly approves the spec.

### Phase 2: TDD Protocol (Red-Green-Refactor)

Once the user approves the spec (e.g., "Approved", "Looks good", "Proceed with TDD"):

1. **RED (Write Failing Tests):**
   - Read the `Given / When / Then` scenarios in `openspec/changes/<feature-name>/specs/`.
   - Write tests in Rust or React/TypeScript corresponding to those scenarios.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - **Verify:** Confirm test failure for the new functionality while any legacy tests remain **GREEN**.
2. **GREEN (Minimal Implementation):**
   - Write the absolute minimum code necessary to satisfy the failing tests.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - Run type checks (`cargo check` or `npx tsc --noEmit`).
   - **Verify:** Confirm all tests pass (100% green) and no compilation/type errors exist.
3. **REFACTOR (Clean & Consolidate):**
   - Clean up code formatting, types, and structure without altering behavior.
   - Run linters (`cargo clippy -- -D warnings` / `npm run lint`).
   - Re-run test suites via CLI to guarantee no regressions.
4. **CONSOLIDATE & ARCHIVE:**
   - Mark completed items in `tasks.md`.
   - Once all scenarios pass, run `openspec archive <feature-name>` to merge the delta into `openspec/specs/`.

### Practical Lessons Learned (SDD + TDD)

#### Archive requires exact header matching
`openspec archive` busca el header exacto del delta en la spec destino. Si el header del delta es `"### Requirement: Pipeline evaluation order (WAF first)"` pero la spec tiene `"### Requirement: Pipeline evaluation order"`, el archive falla. **Los headers del delta deben copiar EXACTAMENTE los de la spec destino.**

#### Si reescribes la spec directamente, no intentes archivar
Si modificaste `openspec/specs/<module>/spec.md` a mano (fuera del mecanismo de archive), el change proposal correspondiente queda huérfano. No se puede archivar porque los headers ya no coinciden. **Solución: eliminar el directorio del change proposal** (`rm -rf openspec/changes/<feature>/`).

#### Cambios en cascada
Eliminar una entidad (ej. Whitelist/Blacklist) puede dejar código muerto en otras partes (ej. `AppError::Conflict`, tests de Conflict). El REFACTOR phase debe incluir la limpieza de estos artefactos. **Siempre ejecutar `cargo clippy -- -D warnings` tras el GREEN phase para detectar código/ variantes no usados.**

#### `openspec archive --yes` no bypassa validación de headers
La flag `--yes` salta la comprobación de tareas incompletas, pero NO la validación de que los headers del delta existan en la spec destino. Si los headers no matchean, el archive igual falla.

#### Mantén openspec artifacts sincronizados con el código
Si implementas un cambio en código pero no actualizas los artifacts de openspec (tasks, proposal), el change proposal queda "stuck" — no se puede archivar ni continuar. **Antes de empezar un nuevo cambio, verifica que no haya cambios activos huerfanos con `openspec list`.**

---

## III. PROJECT CONFIGURATION & CONVENTIONS

### Stack Commands

#### Backend: Rust
- **Test Runner:** `cargo test` (or `cargo nextest run` if available).
- **Type Checking & Linting:** `cargo check` and `cargo clippy -- -D warnings` (enforce zero warnings).
- **Formatting:** `cargo fmt --check`
- **Conventions:**
  - Structs and types placed in domain modules or `src/models/`.
  - Unit tests placed in the same file under `#[cfg(test)]`.
  - Integration and API tests placed in `tests/`.

#### Frontend: React + TypeScript
- **Test Runner:** `npx vitest run` or `npm test -- --watch=false` (single-pass execution).
- **Type Checking:** `npx tsc --noEmit` (mandatory during GREEN/REFACTOR steps).
- **Linting & Formatting:** `npm run lint` / `npx eslint .`
- **Conventions:**
  - Components in `src/components/`, hooks in `src/hooks/`.
  - Component tests colocated as `Component.test.tsx` using `@testing-library/react`.
  - User-centric testing behavior using `@testing-library/user-event` instead of implementation details.

### Custom Repository Rules

- Insert here any specific business logic, database conventions, or custom architectural rules unique to this project.

---

## IV. RESPONSE FORMAT & STATUS MESSAGES

Always prefix your progress updates with the current status tag:

```text
[LEGACY - INSPECT] Creating baseline spec & characterization tests.
[OPENSPEC - DRAFT] Generating change proposal in openspec/changes/...
[OPENSPEC - WAITING] Spec generated. Awaiting user review and approval.
[TDD - RED] Creating tests for scenario <Name> -> Running CLI tests.
[TDD - GREEN] Implementing minimal code -> Running CLI tests & type checks.
[TDD - REFACTOR] Refactoring code -> Running Clippy/ESLint & tests.
[OPENSPEC - ARCHIVE] Archiving change into openspec/specs/.
```


## V. CURRENT PROJECT STATE

### Stack

- **Backend**: Rust + Axum + SQLite (sqlx 0.9, WAL mode)
- **Frontend**: React 19 + TypeScript 7 + Vite 8 + Ant Design 6 + react-router v8
- **Infra**: Docker multi-stage, Podman, `just`, Git Flow

### Estructura

```
watchbeat/
├── backend/          # Rust ([[bin]] + [lib] en Cargo.toml)
│   ├── src/
│   │   ├── main.rs           # Entrypoint + SchedulerManager::spawn()
│   │   ├── lib.rs            # Re-exporta todos los módulos
│   │   ├── config.rs         # Env vars → Config (OIDC obligatorio)
│   │   ├── db.rs             # SQLite + migraciones + CRUD (sqlx)
│   │   ├── auth.rs           # OIDC discovery + JWKS + JWT validation
│   │   ├── models.rs         # Monitor, CheckResult, Notifier, StatusPage, etc.
│   │   ├── template.rs       # Motor de plantillas (minijinja) para notificaciones
│   │   ├── embed.rs          # SPA embebida (include_dir!)
│   │   ├── scheduler.rs      # SchedulerManager + per-monitor tokio timers
│   │   ├── checker/          # mod.rs (http, tcp, ping) + tls.rs
│   │   ├── notifier/         # 8 tipos: telegram, matrix, ntfy, webhook, slack, discord, email, gotify
│   │   └── routes/           # 11 módulos de rutas
│   │       ├── auth_routes   # Login OIDC, callback, logout, me
│   │       ├── monitors      # CRUD + toggle + run-check
│   │       ├── checks        # List checks, timeline, recent global
│   │       ├── notifiers     # CRUD + test
│   │       ├── settings      # Get/set settings
│   │       ├── status        # Dashboard status endpoint
│   │       ├── status_pages  # CRUD status pages + public page
│   │       ├── backup        # Backup DB endpoint
│   │       ├── export_import # Export/import full config JSON
│   │       ├── exports       # Export checks CSV/JSON por monitor
│   │       └── heartbeats    # Heartbeat ping público
│   └── tests/
│       └── db_integration.rs # Tests de integración con SQLite (tempfile)
├── frontend/
│   └── src/
│       ├── App.tsx           # Router + lazy-loaded pages
│       ├── main.tsx          # Entrypoint con ConfigProvider (theme)
│       ├── api/http.ts       # Fetcher genérico con auth JWT
│       ├── store/auth.ts     # JWT en sessionStorage + localStorage
│       ├── hooks/            # useAuth, useSse, useTheme
│       ├── components/       # AppLayout (sidebar + header), MonitorCard
│       └── pages/            # 4 páginas lazy-loaded
│           ├── Dashboard     # Vista general con SSE en tiempo real
│           ├── MonitorDetail # Detalle de monitor + checks + timeline
│           ├── Settings      # Configuración global
│           └── LoginPage     # Login OIDC
├── compose.yml               # Docker Compose canónico
├── docker-compose.yml        # Legacy alias (apunta a compose.yml)
├── Dockerfile                # Multi-stage (frontend-builder → backend-builder → runtime)
├── .justfile                 # Task runner (check, lint, fmt, build, push, gitflow recipes)
├── GIT_FLOW.md               # Convenciones Git Flow
└── PLAN.md                   # Roadmap de features
```

### Comandos esenciales

```bash
## Pre-commit (siempre en este orden)
just check                    # cargo fmt --check + cargo clippy -- -D warnings

## Backend
cd backend && cargo build
cd backend && cargo test      # Tests unitarios + integración
cd backend && cargo test --test db_integration  # Solo integración
cd backend && cargo clippy --all-targets --all-features
cd backend && cargo fmt

## Frontend
cd frontend && npm run dev    # Dev server en :3050, proxy a backend :3055
cd frontend && npm run build  # tsc -b && vite build

## Docker
just build                    # podman build con tag de versión (vampus)
just push                     # podman push con authfile
docker compose -f compose.yml up -d

## Git Flow (vía just)
just gf-feature <name>        # feature/<name> desde development
just gf-finish <name>         # merge --no-ff a development
just gf-release <version>     # release/<version> desde development
just gf-publish <version>     # merge a main + develop + tag
just gf-hotfix <desc>         # hotfix/<desc> desde main
just gf-hotfix-publish <desc> <version>
just gf-graph                 # git log --oneline --graph --all -30
```

### Convenciones

#### Git Flow
- `main` = producción, `development` = integración
- Prefijos: `feature/`, `release/`, `hotfix/`, `support/`
- Siempre `--no-ff` en merges
- Tags semánticas: `v0.1.0`, `v0.2.0`, etc.

#### Commits (Conventional Commits + gitmoji)
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

#### Versionado
- `vampus` tool gestiona la versión en `Cargo.toml`
- `just upgrade` incrementa patch, hace `cargo update`, commitea y tagea

### Detalles técnicos que un agente puede pasar por alto

#### Backend — Scheduler

- **SchedulerManager** en `scheduler.rs` — reemplaza el antiguo bucle polling global. Cada monitor tiene su propio `tokio::time::interval` con `MissedTickBehavior::Skip` (evita bucles si un check se retrasa).
- **Comandos**: `SchedulerCommand` enum (`Spawn`, `Update`, `Remove`, `ReloadNotifiers`) enviados por canal `tokio::sync::mpsc` desde las rutas CRUD.
- **Panic recovery**: `monitor_task` usa `AssertUnwindSafe` + `.catch_unwind()` para reiniciar el task del monitor tras 30s si panic.
- **Caches en memoria por monitor**: checker (`Box<dyn Checker>`), `was_up: bool`, `notifier_ids: Vec<String>` — **0 queries SQLite por check**.
- **Writes reducidos**: solo se inserta en `checks` cuando cambia el estado o cada 10º check (muestreo de latencia). Monitores estables → ~0 writes/min.
- **SSE event**: solo se aloca el JSON si `event_tx.receiver_count() > 0` (sin frontend → 0 allocs).
- **`run_monitor_check()`** acepta `checker: Option<&dyn Checker>`, `was_up: bool`, `notifier_ids: &[String]`, `check_count: u64` y devuelve `bool` (nuevo `is_up`).
- **`last_check_at`**: `AtomicI64` (timestamp unix) en vez de `RwLock<Option<String>>`.

#### Backend — SQLite

- **WAL mode** con `synchronous=NORMAL` (crash-safe con WAL, ~50x más rápido que FULL).
- **Pragmas**: `journal_size_limit=65536`, `cache_size=-8000` (8MB), `busy_timeout=5000`, `temp_store=memory`.
- **Índice cubriente**: `idx_checks_uptime(monitor_id, checked_at, status)` para queries de uptime (index-only scan).
- **`max_connections=4`**, foreign_keys ON.
- **Prometheus eliminado**: no hay `/metrics` endpoint, no hay dependencia `prometheus-client`, no hay timer cada 30s.

#### Backend — HTTP Client

- **`reqwest::Client` global** con `OnceLock` en `checker/mod.rs` — se crea una vez al arrancar, reutilizado en todos los checks HTTP. Pool de conexiones, TLS, DNS cacheados.
- Timeout por request (`.timeout(timeout)`), no por cliente.

#### Backend — Template Engine

- **`template.rs`** — motor de plantillas basado en `minijinja` para notificaciones.
- **`GlobalDefaults`**: struct con templates por defecto (`down`, `latency`, `up`, `expiry`) cargados desde settings DB.
- **`render_notification()`**: función que renderiza una plantilla con contexto del monitor + check result, usando `minijinja` con filtros `json`, `upper`, `lower`.
- **Fallback**: si no hay template personalizado, usa defaults globales; si no hay defaults, usa template hardcoded.

#### Backend — Backup & Export/Import

- **`backup.rs`**: endpoint `POST /api/backup` que crea un backup SQLite vía `VACUUM INTO` en `data_dir/watchbeat.backup.db`.
- **`export_import.rs`**: exporta/importa configuración completa (monitores, notificadores, status pages, settings) como JSON.
- **`exports.rs`**: exporta checks de un monitor en formato CSV o JSON.

#### Backend — Status Pages

- **`status_pages.rs`**: CRUD completo de status pages públicas.
- **`StatusPage`** model: `id`, `slug`, `title`, `description`, `monitors: Vec<String>`, `public: bool`.
- **Ruta pública**: `GET /status/{slug}` — página pública sin autenticación.
- **Rutas protegidas**: `GET/POST /api/status-pages`, `PUT/DELETE /api/status-pages/{id}`.

#### Backend — Heartbeats

- **`heartbeats.rs`**: endpoint público `POST /api/heartbeat/{token}` para monitores tipo `heartbeat`.
- El token identifica al monitor; al recibir un ping, actualiza `last_seen_at`.
- Útil para vigilar cron jobs, backups, o procesos que deben ejecutarse periódicamente.

#### Backend — General

- **OIDC es OBLIGATORIO** — sin proveedor OIDC el binario no arranca (panic en `env_required`).
- **`[[bin]]` + `[lib]`** — los tests de integración importan `watchbeat::db::Database` desde el lib crate.
- **Backend embebe el frontend** — `include_dir!` en `embed.rs` compila `frontend/dist` dentro del binario. Para desarrollo local se usa el proxy de Vite, no el binario embebido.
- **Checker trait** — `#[async_trait]` con `fn check(&self, monitor: &Monitor) -> CheckOutcome`.
- **Notifier dispatch** — en `scheduler.rs` `run_monitor_check()`, dispatch manual por tipo (no usa `NotifierTrait`).
- **Confirmación de caída** — `confirmations_required` + `failed_checks` evitan falsos positivos.
- **Retención configurable** — `settings.retention_days`, default 30, cleanup cada 24h.
- **Rutas públicas** que saltan el middleware JWT: `/auth/*`, `/health`, `/status/{slug}`, `/api/heartbeat/{token}`, `/api/events`.
- **Nullable fields en update**: todos los campos `Option` (`latency_threshold_ms`, `notifier_id`, `message_template_*`, `grace_seconds`) se asignan directamente desde el request (`req.campo`), NO con `.or(existing.campo)` — esto permite que `null` limpie el valor en DB.

#### Frontend — SSE

- **Hook `useSse`** en `hooks/useSse.ts` — conecta `EventSource` a `/api/events?token=<jwt>`, auto-reconnect 5s, cleanup en unmount.
- **Eventos**: tipo `CheckEvent` con `{ type: "check", monitor_id, monitor_name, status, response_time_ms, error_message, checked_at }`.
- **Dashboard**: `load()` con spinner para carga inicial/filtros; `refresh()` silencioso (sin spinner) para SSE y fallback poll 60s.
- **MonitorDetail**: `latestCheck` independiente de la tabla filtrada (que por defecto muestra solo errores). Se actualiza inmediatamente desde el evento SSE + re-fetch de monitor/buckets/checks.
- **Auth**: el token se pasa como query param `?token=` porque `EventSource` no soporta headers personalizados. También soporta `Authorization: Bearer`.

#### Frontend — General

- **react-router v8** — imports desde `"react-router"`, NO `"react-router-dom"`.
- **Vite proxy** — `/api`, `/auth`, `/health` → `localhost:3055` en dev.
- **Auth** — JWT en `sessionStorage` + `localStorage` (clave `watchbeat_token`).
- **Dark mode** — `localStorage` clave `watchbeat-theme`, `ConfigProvider` con `darkAlgorithm`.
- **Sin tests de frontend** — no hay test runner configurado.
- **Lazy loading** — todas las páginas con `React.lazy()` + `Suspense`.
- **Formularios**: campos opcionales (`latency_threshold_ms`, `notifier_id`) se inicializan con `?? undefined` (no `null`) para que Ant Design los muestre vacíos. En submit, `undefined` se convierte a `null` para enviar a la API.
- **MonitorCard**: componente de tarjeta individual para cada monitor en el Dashboard.

#### Docker

- **Multi-stage**: `node:23-alpine` (frontend) → `rust:alpine3.23` (backend) → `alpine:3.23` (runtime).
- **El backend necesita el frontend compilado** — `COPY --from=frontend-builder` antes de compilar Rust.
- **Healthcheck**: `wget --spider http://localhost:3055/health`.
- **Puerto**: 3055 (configurable via `WATCHBEAT_PORT`).
- **compose.yml** es el canónico; `docker-compose.yml` es alias legacy.
- **Volumen**: `watchbeat_data` para persistencia de SQLite en `/app/data`.
- **Recursos**: límite 256M memory, reserva 64M.

#### Testing

- Tests unitarios inline (`#[cfg(test)] mod tests`) en `checker/mod.rs`, `config.rs`.
- Tests de integración en `backend/tests/db_integration.rs` (usa `tempfile`).
- No hay tests de frontend.
- Para tests de integración: `cargo test --test db_integration`.

#### Archivos que no deberían editarse

- `docker-compose.yml` — alias legacy, editar `compose.yml`.
- `GIT_FLOW.md` — duplicado de info en README, mantener sincronizado.

### Referencias

- `PLAN.md` — roadmap de features pendientes.
- `GIT_FLOW.md` — convenciones de ramas y commits.
- `watchbeat.env.example` — todas las variables de entorno documentadas.
