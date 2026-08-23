# WatchBeat — Plan v2: Features de producción

Plan de expansión de WatchBeat para convertirlo de MVP a herramienta de producción.

## Resumen de features

| # | Feature | Área | Complejidad | Valor |
|---|---------|------|:-----------:|:-----:|
| 1 | SSL/TLS expiry check | Backend | Baja | 🔥 |
| 2 | Confirmación de caída (retry/backoff) | Backend | Media | 🔥 |
| 3 | Validación de contenido (expected_body + regex) | Backend | Baja | 🔥 |
| 4 | Multi-notifier por monitor | Backend + Frontend | Media | 🔥 |
| 5 | Status page pública | Backend + Frontend | Media | 🔥 |
| 6 | Heartbeat API | Backend + Frontend | Media | 🔥 |
| 7 | SSE en vivo para dashboard | Backend + Frontend | Media | ⭐ |
| 8 | Gráfica de latencia real (p95) | Frontend | Media | ⭐ |
| 9 | Dark mode | Frontend | Baja | ⭐ |
| 10 | Export CSV/JSON histórico | Backend + Frontend | Baja | ⭐ |
| 11 | Retención configurable en UI | Backend + Frontend | Baja | 🧹 |
| 12 | Backup SQLite (WAL checkpoint) | Backend | Baja | 🧹 |
| 13 | Docker Compose de ejemplo | Infra | Baja | 🧹 |
| 14 | OpenAPI/Swagger | Backend | Media | 🧹 |

## Fase 1 — Backend hardening (1 sesión)

### 1. SSL/TLS expiry check

Nuevo checker `tls` que conecta al host:puerto y lee el certificado:

```rust
// checker/tls.rs
pub struct TlsChecker;

// -> CheckOutcome con:
//   status: "up" | "warning" | "down"
//   extra: { expires_at, days_left }
```

- `expires_in_days` configurable por monitor (default 14)
- Si `days_left < expires_in_days` → `warning` (no es caída, pero se marca)
- En `MonitorSummary` añadir `cert_expires_at`, `cert_days_left`
- Dashboard: badge naranja "⚠️ cert expira en 10 días"

### 2. Confirmación de caída

Evitar falsos positivos por timeouts puntuales:

- Añadir a `monitors`: `confirmations_required INTEGER DEFAULT 0`
- Si un check falla y `confirmations_required > 0`, no se marca DOWN hasta N fallos consecutivos
- Estado intermedio: `flapping` / `checking` en el summary
- El scheduler re-check con backoff: 60s, 5min, 15min

### 3. Validación de contenido

En `config_json` de monitores HTTP:

```json
{
  "expected_status": 200,
  "expected_body": "error|panic|500",
  "body_is_regex": true
}
```

- `expected_body`: substring o regex según `body_is_regex`
- Si el body no matchea → DOWN con mensaje "Contenido no esperado"
- Limitar body leído a 64KB para no tragar páginas enormes

## Fase 2 — Notificaciones y status page (1 sesión)

### 4. Multi-notifier por monitor

Hoy `monitors.notifier_id` es 1:1. Cambiar a tabla N:M:

```sql
CREATE TABLE monitor_notifiers (
    monitor_id TEXT NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    notifier_id TEXT NOT NULL REFERENCES notifiers(id) ON DELETE CASCADE,
    PRIMARY KEY (monitor_id, notifier_id)
);
```

- Frontend: en el modal de monitor, `Select multiple` de notificadores
- El scheduler itera todos los notifiers del monitor y envía a cada uno

### 5. Status page pública

- Tabla `status_pages`:
```sql
CREATE TABLE status_pages (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    monitors TEXT NOT NULL,  -- JSON array de monitor_ids
    public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
```
- Ruta pública: `GET /status/<slug>` — HTML embebido (sin auth) con:
  - Estado global (UP/DOWN por monitor)
  - Uptime 90d
  - Timeline compacto
- CSS inline, sin JS (o JS vanilla mínimo) — embebible en iframe
- Frontend: sección Status Pages para crear/editar slug, título, monitores

### 6. Heartbeat API

Para vigilar cron jobs / backups:

- Tabla `heartbeats`:
```sql
CREATE TABLE heartbeats (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    grace_seconds INTEGER NOT NULL DEFAULT 3600,
    last_seen_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    notifier_id TEXT,
    created_at TEXT NOT NULL
);
```
- Ruta pública: `POST /api/heartbeat/<token>` → actualiza `last_seen_at`
- Scheduler: si `now - last_seen_at > grace_seconds` → status `missing` + notificación
- Frontend: sección Heartbeats con CRUD + URL a copiar

## Fase 3 — Frontend avanzado (1 sesión)

### 7. SSE en vivo

- Endpoint `GET /api/events` (SSE, auth)
- Broadcast de cada check completado → dashboard se actualiza al momento
- Frontend: `EventSource` con fallback a polling 30s
- Implementación: `tokio::sync::broadcast` + `axum::response::sse`

### 8. Gráfica de latencia real

- Sustituir las barras de `MonitorDetail` por línea de latencia
- Añadir percentil p95 calculado en backend: `GET /api/monitors/{id}/latency?days=7` → `{ points: [{t, p50, p95}], p95_7d }`
- Frontend: componente SVG custom (sin librería pesada) o echarts

### 9. Dark mode

- `ConfigProvider` con `theme.darkAlgorithm`
- Toggle en header, persistencia en localStorage
- Ajustar colores de MonitorCard/Timeline

## Fase 4 — Calidad y ops (1 sesión)

### 10. Export CSV/JSON
- `GET /api/monitors/{id}/export?format=csv|json&days=30`
- Botón en MonitorDetail

### 11. Retención configurable
- `settings.retention_days` editable en UI (Ajustes)
- El scheduler usa el valor en vez de hardcode 30

### 12. Backup SQLite
- Comando/scheduler: `VACUUM INTO` para snapshot consistente
- Retención de N backups
- Opcional: `BACKUP_CRON` env var

### 13. Docker Compose
- `docker-compose.yml` con volumen, healthcheck, restart policy
- Env vars documentadas

### 14. OpenAPI/Swagger
- `utoipa` para generar spec del router
- `/docs` con Swagger UI embebida (swagger-ui crate o CDN)

## Calendario

| Sesión | Fase | Entregable |
|--------|------|-----------|
| 1 | Backend hardening | SSL + confirmación + body check |
| 2 | Notificaciones + status page | Multi-notifier + status page + heartbeat |
| 3 | Frontend avanzado | SSE + latencia + dark mode |
| 4 | Calidad y ops | Export + retención + backup + compose + swagger |

## Quality Gates

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test` — todo verde
- `pnpm build` — sin errores TS
- Migraciones nuevas en `backend/migrations/` con naming `YYYYMMDDHHMMSS_<name>.sql`
- Tests para cada feature nueva (unit + integración)
- Sin romper los 49 tests existentes
