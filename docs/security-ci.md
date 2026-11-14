# CI de seguridad

Workflow `.github/workflows/security.yml` introducido en el issue #61. Antes no existía ninguna
comprobación automatizada de vulnerabilidades de dependencias, y Dependabot se quedaba stale y
no detectaba las vulnerabilidades reales que sí veía `cargo audit`.

## Qué hace

| Job | Comando | Directorio |
|---|---|---|
| `rust` | `cargo audit` | `backend/` |
| `frontend` | `npm audit --audit-level=high` | `frontend/` |

**Triggers:** `push` y `pull_request` a `main` y `development`, más una ejecución programada
semanal (`schedule.cron = "0 6 * * 1"`, lunes 06:00 UTC). `permissions: contents: read`.

Ambos jobs fallan el build ante vulnerabilidades detectadas.

## Excepción documentada

`backend/.cargo/audit.toml` contiene la única incidencia ignorada:

```toml
[advisories]
ignore = ["RUSTSEC-2026-0459"]
```

`RUSTSEC-2026-0459` afecta a `libsqlite3-sys` (C SQLite CVE-2026-119375), que se arrastra
transitivamente vía `sqlx`. Está corregida en `libsqlite3-sys >= 0.39.0`, pero sqlx fija 0.37.0,
así que la actualización no es alcanzable desde este proyecto. **Re-evaluar en cada subida de
sqlx y eliminar el ignore en cuanto se pueda adoptar una versión parcheada.** La justificación
y el enlace al RUSTSEC están en el propio fichero y referenciados desde el workflow.

`cargo-audit` lee `.cargo/audit.toml` automáticamente al invocarse desde `backend/`.

## Decisiones

- `cargo install cargo-audit --locked` en lugar de `rustsec/audit-check@v2`: se necesita ejecutar
  dentro de `backend/` para que aplique `audit.toml`, y `rustsec/audit-check` no expone
  `working-directory` de forma directa. `--locked` garantiza reproducibilidad y el control
  explícito del exit code.
- `npm ci` antes del audit: el build usa `npm` (hay `package-lock.json`).
- Node 23 en CI, coherente con el `node:23-alpine` del Dockerfile.

## Cómo ejecutarlo en local

```bash
cd backend && cargo audit                       # respeta .cargo/audit.toml
cd frontend && npm ci && npm audit --audit-level=high
```

## Estado actual

- `cargo audit`: 0 vulnerabilidades, 2 warnings `unsound` (rustix, tokio) permitidos.
- `npm audit --audit-level=high`: 0 vulnerabilidades.