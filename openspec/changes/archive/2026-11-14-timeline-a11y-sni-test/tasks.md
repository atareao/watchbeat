# Tasks

## 1. RED — tests que fallan

- [x] 1.1 Añadir `rcgen` a `[dev-dependencies]` de `backend/Cargo.toml`.
- [x] 1.2 Test en `checker/tls.rs`: servidor TLS local con cert para `example.test` que captura el
      ClientHello; asserta que el SNI recibido es `example.test`.
- [x] 1.3 Test en `checker/tls.rs`: target IP literal → handshake sin extensión SNI.
- [x] 1.4 `cargo test` en RED (fallan los nuevos, verdes los existentes).

## 2. GREEN — implementación mínima

- [x] 2.1 `frontend/src/theme/statusStyles.ts`: mapa único `estado → { color, pattern, label,
      icon }` (incluye `warning`).
- [x] 2.2 `HealthLatencyChart` / `HeartbeatChart`: aplicar patrón + `role="img"` + `tabIndex` +
      `aria-label` + `Tooltip trigger=["hover","focus"]` + foco visible.
- [x] 2.3 Reutilizar el token en `MonitorCard.tsx` y `Dashboard.tsx`.
- [x] 2.4 (Backend) ajustar `tls.rs` solo si el test RED revela que el SNI no se envía como se
      espera.
- [x] 2.5 `cargo test` verde; `npx tsc --noEmit` y `npm run build` verdes.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets --all-features -- -D warnings` limpios.
- [x] 3.2 `npm run lint` no existe en el repo (sin eslint); N/A — verificado `tsc --noEmit` + build.
- [x] 3.3 Re-ejecutar las suites.

## 4. Cierre

- [x] 4.1 `docs/timeline-a11y-sni-test.md`.
- [x] 4.2 `@rust-reviewer` y `@react-reviewer` sobre el diff.
- [x] 4.3 PR a `development` con `Closes #63`.
- [x] 4.4 Archivar change y renombrar `plans/PLAN-016.md` → `-ARCHIVED.md`.
