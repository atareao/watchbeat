# Vigilatrs — Plan de implementación

Monitor de uptime auto-hosteado con backend Rust + Axum y frontend React + Ant Design.

## 1. Nombre y estructura

**Proyecto:** `vigilatrs` — de *vigilar* + *rs*
**Repo:** `github.com/atareao/vigilatrs`
**Estructura:**

```
vigilatrs/
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Entrypoint, router, scheduler loop
│       ├── config.rs         # Config desde env vars
│       ├── db.rs             # SQLite (rusqlite) + migraciones
│       ├── models.rs         # Tipos: Monitor, CheckResult, NotifierConfig
│       ├── auth.rs           # OIDC / JWT (mismo patrón que populatrs)
│       ├── middleware.rs     # Auth middleware
│       ├── embed.rs          # SPA embed (include_dir!)
│       ├── checker/
│       │   ├── mod.rs        # Checker trait + dispatch
│       │   ├── http.rs       # HTTP(S) check (reqwest)
│       │   ├── tcp.rs        # TCP port check
│       │   └── ping.rs       # ICMP ping (opcional — requiere privilegios)
│       ├── notifier/
│       │   ├── mod.rs        # Notifier trait + dispatch
│       │   └── telegram.rs   # Notificador Telegram
│       └── routes/
│           ├── mod.rs        # Router builder
│           ├── monitors.rs   # CRUD monitores
│           ├── checks.rs     # Histórico checks, timeline
│           ├── notifiers.rs  # CRUD notificadores
│           ├── status.rs     # Dashboard status
│           └── auth_routes.rs# Login / callback OIDC
├── frontend/
│   ├── package.json          # React 19 + Ant Design 5 + Vite 6
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── theme.ts          # Tema Ant Design
│       ├── api/
│       │   └── http.ts       # Fetcher genérico (auth, JSON)
│       ├── store/
│       │   └── auth.ts       # Auth store (zustand/simple)
│       ├── pages/
│       │   ├── Dashboard.tsx  # Grid monitores + estado global
│       │   ├── Monitors.tsx   # CRUD monitores
│       │   ├── MonitorDetail.tsx # Timeline + histórico + gráfica
│       │   ├── Notifiers.tsx  # CRUD notificadores
│       │   ├── LoginPage.tsx
│       │   └── Settings.tsx   # Config general
│       └── components/
│           ├── AppLayout.tsx  # Layout con sidebar
│           ├── MonitorCard.tsx# Card individual en dashboard
│           ├── Timeline.tsx   # Línea temporal de checks
│           └── StatusBadge.tsx# UP/DOWN/ERROR con colores
├── Dockerfile                # Multi-stage build (como populatrs)
├── GIT_FLOW.md               # Mismo patrón que populatrs
├── cliff.toml                # Git cliff config
├── PLAN.md                   # Este archivo
└── vigilatrs.env.example     # Env vars de ejemplo
```

## 2. Modelo de datos (SQLite)

### Tabla `monitors`

```sql
CREATE TABLE monitors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    monitor_type TEXT NOT NULL,        -- 'http' | 'tcp' | 'ping'
    target TEXT NOT NULL,              -- URL, host:port, IP
    config_json TEXT NOT NULL,         -- extras: method, headers, body, expected_status, etc.
    interval_seconds INTEGER NOT NULL DEFAULT 300,  -- cada 5 min por defecto
    timeout_seconds INTEGER NOT NULL DEFAULT 30,
    enabled INTEGER NOT NULL DEFAULT 1,
    notifier_id TEXT,                  -- FK al notificador (opcional)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### Tabla `checks`

```sql
CREATE TABLE checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    status TEXT NOT NULL,             -- 'up' | 'down' | 'error'
    status_code INTEGER,             -- HTTP status code (si aplica)
    response_time_ms INTEGER,        -- Tiempo de respuesta en ms
    error_message TEXT,              -- Mensaje si falló
    checked_at TEXT NOT NULL         -- ISO 8601
);

CREATE INDEX idx_checks_monitor_id ON checks(monitor_id, checked_at DESC);
CREATE INDEX idx_checks_checked_at ON checks(checked_at);
```

### Tabla `notifiers`

```sql
CREATE TABLE notifiers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    notifier_type TEXT NOT NULL,      -- 'telegram' | 'email' | ...
    config_json TEXT NOT NULL,        -- bot_token, chat_id, etc.
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### Tabla `settings` (genérica, como populatrs)

```sql
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

## 3. API Routes

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET` | `/health` | Health check público |
| `GET` | `/auth/login` | Inicio OIDC |
| `GET` | `/auth/callback` | Callback OIDC |
| | | |
| `GET` | `/api/me` | Usuario actual |
| | | |
| `GET` | `/api/monitors` | Lista monitores |
| `POST` | `/api/monitors` | Crear monitor |
| `PUT` | `/api/monitors/{id}` | Actualizar monitor |
| `DELETE` | `/api/monitors/{id}` | Eliminar monitor |
| `PATCH` | `/api/monitors/{id}` | Toggle enable/disable |
| `POST` | `/api/monitors/{id}/check` | Ejecutar check manual |
| | | |
| `GET` | `/api/monitors/{id}/checks` | Histórico checks (paginado) |
| `GET` | `/api/monitors/{id}/timeline` | Timeline resumido para gráfica (últimas 24h / 7d) |
| | | |
| `GET` | `/api/notifiers` | Lista notificadores |
| `POST` | `/api/notifiers` | Crear notificador |
| `PUT` | `/api/notifiers/{id}` | Actualizar |
| `DELETE` | `/api/notifiers/{id}` | Eliminar |
| `POST` | `/api/notifiers/{id}/test` | Enviar notificación de prueba |
| | | |
| `GET` | `/api/status` | Dashboard: resumen global |
| `GET` | `/api/checks/recent` | Últimos checks globales (feed en dashboard) |

## 4. Lógica de negocio (backend)

### 4.1 Checker Engine

```rust
#[async_trait]
trait Checker: Send + Sync {
    async fn check(&self, monitor: &Monitor) -> CheckResult;
}

struct CheckResult {
    status: CheckStatus,   // Up | Down | Error
    status_code: Option<u16>,
    response_time_ms: u64,
    error_message: Option<String>,
}
```

Implementaciones:

- **HTTP:** reqwest GET/HEAD al target con timeout. Status code 2xx/3xx = UP. Opción de expected_body regex.
- **TCP:** tokio::net::TcpStream connect con timeout. Conexión exitosa = UP.
- **Ping:** tokio::process::Command("ping", ...) con timeout. Respuesta = UP. (⚠️ requiere CAP_NET_RAW o setuid)

### 4.2 Scheduler Loop

Mismo patrón que populatrs: un `tokio::spawn` con loop infinito que:

1. Cada tick carga monitores habilitados de BD
2. Para cada monitor, si `last_check_at + interval < now`, ejecuta check
3. Guarda resultado en `checks`
4. Si cambió de estado (was_up → now_down), dispara notificación
5. Espera 10s y repite (no sleep largo — polling fino)

### 4.3 Notificador

```rust
#[async_trait]
trait Notifier: Send + Sync {
    async fn notify(&self, monitor: &Monitor, result: &CheckResult, was_up: bool) -> Result<()>;
}
```

Implementación inicial: **Telegram**. Configuración: bot_token + chat_id.

- Si el monitor estaba UP y pasa a DOWN → *"🔴 DOWN — Monitor X no responde en https://..."*
- Si estaba DOWN y pasa a UP → *"🟢 UP — Monitor X responde de nuevo tras 15m de caída"*

## 5. Frontend (React + Ant Design + TS)

### 5.1 Dashboard

Grid de tarjetas (Monitors → `MonitorCard`):

```
┌─────────────────────────────────────────────────┐
│  🟢 vigilatrs.local       ⚡ 45ms  Último: 08:52 │
│  🔴 atareao.es             ⏰ 0ms  Último: 08:45 │
│  🟢 quill.local           ⚡ 12ms  Último: 08:51 │
│  🟡 api.external.com       ⚡ 120ms Último: 08:48 │
└─────────────────────────────────────────────────┘
```

- Estado con icono: 🟢 UP / 🔴 DOWN / 🟡 ERROR / ⚪ PAUSED
- Latencia
- Último check (hace X minutos)
- Uptime % (últimos 7 días / 30 días)
- Barra de estado semanal como sparkline minimalista

### 5.2 MonitorDetail

Página de detalle de un monitor:

- **Header:** nombre, target, tipo, estado actual, botón "Check now"
- **Uptime stats:** 24h / 7d / 30d
- **Timeline:** gráfica de barras verde/roja con latencia superpuesta (Ant Design Charts o custom SVG)
- **Histórico:** tabla paginada de checks con status, latencia, timestamp

### 5.3 Monitors CRUD

Formulario para crear/editar monitor:

| Campo | Tipo | Descripción |
|-------|------|-------------|
| Nombre | Text | Identificador |
| Tipo | Select | HTTP, TCP, Ping |
| Target | Text | URL / host:port / IP |
| Método HTTP | Select | GET, HEAD, POST (solo HTTP) |
| Intervalo | Number | Segundos entre checks |
| Timeout | Number | Timeout por check |
| Notificador | Select | Opcional, enlace al notifier |
| Habilitado | Switch | |

### 5.4 Notifiers CRUD

Formulario para crear/editar notificador:

| Campo | Tipo | Descripción |
|-------|------|-------------|
| Nombre | Text | Identificador |
| Tipo | Select | Telegram (de momento) |
| Bot Token | Password | Token del bot |
| Chat ID | Text | Chat/group ID |
| Habilitado | Switch | |

### 5.5 Layout

Sidebar con:
- Dashboard (🏠)
- Monitors (📡)
- Notificadores (🔔)
- Ajustes (⚙️)

## 6. Config (env vars)

```bash
# Server
HOST=0.0.0.0
PORT=3055
DATA_DIR=./data
DATABASE_URL=./data/vigilatrs.db
TIMEZONE=Europe/Madrid
RUST_LOG=info

# OIDC (opcional en dev)
OIDC_ISSUER_URL=
OIDC_CLIENT_ID=
OIDC_CLIENT_SECRET=
OIDC_REDIRECT_URL=http://localhost:3055/auth/callback
```

## 7. Implementación por fases

### Fase 1 — Esqueleto (1 sesión)
- [ ] Inicializar proyecto Rust con Cargo
- [ ] Config + DB + migraciones
- [ ] Models (Monitor, CheckResult, Notifier)
- [ ] Auth OIDC (copy-paste adaptado de populatrs)
- [ ] main.rs con router básico + health
- [ ] Frontend: Vite + React + Ant Design + Layout + LoginPage
- [ ] Frontend: Dashboard.tsx mock (tarjetas estáticas)
- [ ] Docker multi-stage build

### Fase 2 — Checkers (1 sesión)
- [ ] Checker trait + HTTP checker
- [ ] TCP checker
- [ ] Scheduler loop persistente
- [ ] API CRUD monitors
- [ ] Frontend: Monitors.tsx + Monitordetail.tsx + MonitorCard

### Fase 3 — Histórico (1 sesión)
- [ ] API checks históricos + timeline
- [ ] Frontend: Tabla histórica + Timeline componente
- [ ] Dashboard con datos reales

### Fase 4 — Notificaciones + polish (1 sesión)
- [ ] Notifier trait + Telegram implementación
- [ ] API CRUD notifiers
- [ ] Frontend: Notifiers.tsx
- [ ] Settings page
- [ ] Estado down→up / up→down con notificación
- [ ] Limpieza automática de checks viejos

### Fase 5 — Chapa y pintura (1 sesión)
- [ ] Cálculo uptime % en backend
- [ ] Gráfica timeline con latencia
- [ ] Git Flow + CI + CHANGELOG
- [ ] README.md

## 8. Git Flow

Mismo patrón que populatrs:
- `main` — producción
- `development` — integración
- `feature/*` — features
- Conventional commits con gitmoji
- CI: fmt, clippy, build, test

## 9. Quality Gates

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
- `pnpm build && pnpm test`
- Sin warnings, sin dependencias no usadas