# Actualización de dependencias de seguridad (#59)

- **Issue**: [#59](https://github.com/atareao/watchbeat/issues/59)
- **Rama**: `feature/security-deps-update`

## Qué es

Actualización de dependencias para eliminar las vulnerabilidades detectadas por
`cargo audit` y alinear el frontend con los lockfiles ya parcheados, de modo que
las alertas de Dependabot dejen de ser reales.

## Cambios

### Rust

| Crate | Antes | Después | Motivo |
|---|---|---|---|
| `jsonwebtoken` | 11.1.0 | 12.0.0 | Arrastra `rsa` 0.9 → 0.10 |
| `rsa` | 0.9.10 | 0.10.0 | RUSTSEC-2023-0071 (Marvin) |
| `crypto-bigint` | 0.5.5 | 0.7.11 | RUSTSEC-2026-0387 |
| `ctutils` | 0.4.3 | 0.4.8 | RUSTSEC-2026-0385 |
| `rustls-webpki` | 0.103.15 | 0.103.17 | RUSTSEC-2026-0382 |
| `webpki-roots` | 1.0.9 | 1.0.12 | RUSTSEC-2026-0381 |
| `webpki-root-certs` | 1.0.9 | 1.0.12 | RUSTSEC-2026-0383 |

`jsonwebtoken` 12 mantiene la API usada (`decode`, `Algorithm`, `DecodingKey`,
`Validation`), así que no hubo cambios de código.

### Frontend

- `package.json`: `vite` de `^8.2.2` a `^8.3.8` (mínimo parcheado).
- `package-lock.json` regenerado: `vite` 8.3.8, `source-map-js` 1.2.2.
- `pnpm-lock.yaml` ya estaba en 8.3.8 / 1.2.2.

## Verificación

```bash
cd backend && cargo audit      # solo RUSTSEC-2026-0459; +2 warnings unsound (ver abajo)
cd backend && cargo test       # 266 + 9 + 45 + 8 + 3 verdes
cd frontend && npm audit       # 0 vulnerabilidades
cd frontend && npx tsc --noEmit && npm run build
```

## Pendiente upstream: `libsqlite3-sys` (RUSTSEC-2026-0459)

`sqlx` 0.9.0 (última versión publicada) fija `libsqlite3-sys` 0.37.0, afectado
por CVE-2026-119375 en el SQLite embebido. El fix (`>=0.39.0`) exige una versión
de `sqlx` que aún no existe.

Opciones evaluadas:

1. **Esperar** a que `sqlx` publique una versión con `libsqlite3-sys >=0.39`.
   Es la opción elegida: no hay parche compatible sin cambiar de crate.
2. **`sqlite-unbundled`** + SQLite del sistema: `cargo audit` seguiría marcando
   la versión del crate, y cambia el modelo de despliegue (dependencia de la
   `libsqlite3` de Alpine). Se evaluará si el CVE gana criticidad.

Impacto actual: bajo. SQLite es local, el archivo no lo controla un atacante
remoto y la superficie es la API propia autenticada.

## Warnings `unsound` (no son vulnerabilidades)

`cargo audit` también emite dos avisos informativos preexistentes:

- `rustix` 1.1.5 · RUSTSEC-2026-0486 (`xattr` con buffer de longitud cero).
- `tokio` 1.53.2 · RUSTSEC-2026-0386 (`AsyncFd::new` e I/O safety).

No introducen vulnerabilidades explotables en WatchBeat y no rompen el build.
Se revisarán en las próximas actualizaciones de dependencias.

## MSRV

`jsonwebtoken` 12 requiere Rust ≥1.88. La imagen `rust:alpine3.23` del Dockerfile
está muy por encima (local: 1.97.0), así que no hay acción.

## Seguimiento

- Revisar `cargo audit` en cada release; cuando `sqlx` suba de versión, actualizar.
- Las 18 alertas de Dependabot (vite/source-map-js/rustls) eran **stale**: `main`
  ya tenía las versiones parcheadas desde v0.15.2. Se cierran con la evidencia de
  `npm audit` y `cargo audit`.
