# Changelog
## [0.11.0] - 2026-08-30

### Documentation

- Rewrite README with comprehensive project documentation
- Add self-hoster deployment guide

### Features

- Add status filter to checks history
- Add status filter to checks history + documentation overhaul

### Miscellaneous Tasks

- Update frontend build artifacts
- Bump version to v0.11.0

### Other

- V0.11.0

### Styling

- Update branding assets and favicons
## [0.9.0] - 2026-08-29

### Bug Fixes

- Añadir token/grace_seconds/last_seen_at a queries SQL y MonitorSummary
- StopPropagation en items del menú ⋮ de MonitorCard
- StopPropagation en check, toggle y delete de MonitorCard
- MonitorCard misma altura para heartbeat, spinner sin desplazar grid, eliminar HeartbeatCard.tsx
- WatchBeat clickeable a dashboard, íconos por tipo, toggle en heartbeats, stopPropagation, editar abre modal
- Eliminar scheduler info del dashboard (no relevante con heartbeats)
- StopPropagation en botones, card click navega a detalle (monitores) o editar (heartbeats)
- Consolidation_loop reads full period data and runs on startup
- Add dominant_status to consolidated buckets response
- Show uptime with 1 decimal instead of Math.round()
- Heartbeat detail view - status from grace period, health chart, binary pulse bars
- Dashboard status counts heartbeats by grace period, not last check status
- Limit consolidated buckets to last 60 per period
- Limit consolidated buckets to last 60 per period (consolidation creates new timestamps each run)
- Prune old consolidated buckets after each insert cycle to prevent unbounded growth

### Features

- Show heartbeats alongside monitors in unified dashboard
- Merge heartbeats in dashboard into development
- Dashboard mejorado — stats unificadas, heartbeat modal inline, ruta heartbeats eliminada
- Stats de latencia, uptime 24h/30d/1a y caducidad certificado en MonitorDetail
- Add consolidated_metrics schema and model
- Add consolidated_metrics CRUD methods
- Add hourly consolidation loop for metrics
- Route 6h+ ranges to consolidated_metrics, keep 1h real-time
- Make retention_days configurable via WATCHBEAT_RETENTION_DAYS
- Change consolidation to 60 buckets and enforce minimum 60s interval

### Miscellaneous Tasks

- Sync vampus version to 0.8.0

### Performance

- Move cleanup_old_checks from scheduler loop to hourly consolidation loop

### Refactor

- Polish heartbeat cards & dashboard integration
- Heartbeat como tipo de monitor en dashboard — grid único, modal único, DashboardItem unificado
- Cards clickables navegan a monitor detail, stopPropagation en botones
- Eliminar tabla heartbeats — heartbeat como tipo de monitor
- MonitorCard unificado sin HeartbeatView, Dashboard sin fetchHeartbeats
- Settings con tabs (General/Notificadores/Status Pages), AppLayout top bar, limpieza rutas
- Settings con tabs de un solo nivel (sin anidamiento)

### Revert

- Remove take(60) limit - consolidated always has exactly 60 buckets

### Styling

- Fmt fixes after consolidation tasks
## [0.8.0] - 2026-08-27

### Features

- Unify dashboard and monitors into single view
- Merge unified dashboard into development

### Miscellaneous Tasks

- Bump version to 0.8.0
- Update Cargo.lock for v0.8.0

### Other

- V0.8.0
## [0.7.2] - 2026-08-27

### Bug Fixes

- Latency notification fallback + default 24h view
- Latency notification fallback + default 24h view
- Default-view-24h
- Pagination-and-fixes

### Features

- Paginación completa + JWKS rotation + Matrix fix + template placeholders
- Paginación completa + JWKS rotation + Matrix fix + template placeholders

### Miscellaneous Tasks

- Bump version to 0.7.2
## [0.7.1] - 2026-08-26

### Other

- Settings auth, monitor config field name, settings tabs (#8)
## [0.5.0] - 2026-08-25

### Other

- V0.5.0
## [0.4.2] - 2026-08-25

### Bug Fixes

- Placeholders SQL en get_timeline_buckets
- Merge sql-placeholders into develop
- Buckets adaptativos basados en rango real de datos
- Strftime con substr para timestamp limpio + span desde Rust
- Bucketing en Rust en lugar de SQL
- Comparación directa de strings ISO 8601 en bucketing
- Usar Link de react-router en lugar de <a href=#/...>
- Scheduler robusto con auto-recuperación y checks paralelos
- Sql-placeholders

### Features

- Merge feature/unify-charts into develop
- 80 bloques fijos con gris para sin datos
- Extraer info TLS en checker HTTP
- Mejorar LoginPage con theme-aware y toggle
- Organizar formulario de monitores en tabs

### Miscellaneous Tasks

- Bump version to 0.4.2
- Bump version to 0.5.0
- Bump version to 0.6.0

### Refactor

- Unificar Health Chart y Latencia en un solo gráfico
- Extraer ThemeProvider a contexto React
## [0.4.1] - 2026-08-25

### Bug Fixes

- Placeholders SQL en get_timeline_buckets

### Features

- Health chart con buckets adaptativos y selector de rango
- Merge feature/timeline-buckets into develop

### Miscellaneous Tasks

- Bump version to 0.4.1

### Other

- V0.4.1
## [0.4.0] - 2026-08-25

### Features

- Monitor detail redesign + name uniqueness + indexes
- Release 0.4.0

### Miscellaneous Tasks

- Bump version to 0.4.0
## [0.3.0] - 2026-08-25

### Features

- Monitor review — monitor_notifiers table, checker tests, config fields

### Miscellaneous Tasks

- Bump version to 0.3.0
## [0.2.1] - 2026-08-25

### Bug Fixes

- Heartbeat ping endpoint returns 401 (auth middleware)
- Heartbeat ping endpoint returns 401 (auth middleware)

### Features

- Notifier review — factory completa, test multi-tipo, 43 tests, vista simplificada

### Miscellaneous Tasks

- Bump version to 0.2.1

### Other

- V0.2.0
## [0.2.0] - 2026-08-25

### Documentation

- Plan v2 features de producción

### Features

- Vigilatrs v0.1 — uptime monitor con Rust+React
- *(fase1)* SSL checker + confirmación caída + body validation
- *(fase2)* Multi-notifier, status pages y heartbeats
- *(frontend-fase2)* Status pages, heartbeats y multi-notifier
- *(fase3)* SSE en vivo, dark mode y gráfica de latencia
- *(fase4)* Export CSV/JSON, backup, retención configurable, compose
- 8 notificadores — Telegram, Matrix, ntfy, Webhook, Slack, Discord, Email, Gotify
- Prometheus metrics, tags, login branding, connectivity check
- Update dependencies — Rust (tokio 1.53, reqwest 0.13, sqlx 0.9, jsonwebtoken 11, base64 0.23) + Frontend (antd 6, vite 8, ts 7)
- Update dependencies — Rust + Frontend

### Miscellaneous Tasks

- Add gitignore + dockerignore, remove tracked build artifacts
- Remove unused migrations dir (inline schema in db.rs)
- Bump version to 0.2.0

### Refactor

- Sqlx + tests (49 tests, 0 warnings)
- Restore sqlx migrations (migrate! instead of inline schema)
