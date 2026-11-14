## ADDED Requirements

### Requirement: Auditoría de dependencias en CI

La integración continua SHALL ejecutar `cargo audit` sobre el backend Rust y `npm audit --audit-level=high` sobre el frontend en cada `push` y `pull_request` a `main` y `development`, además de una ejecución programada semanal. Los jobs SHALL fallar cuando detecten una vulnerabilidad conocida. Solo se admitirá una excepción (`ignore`) para una vulnerabilidad bloqueada por una dependencia upstream, con su justificación y referencia en el propio workflow y en la documentación.

#### Scenario: PR con dependencia vulnerable

- **Given** una rama con una dependencia que introduce una vulnerabilidad conocida
- **When** se abre un `pull_request` a `development`
- **Then** el workflow `security` falla y bloquea el merge

#### Scenario: Ejecución semanal

- **Given** el workflow configurado con `schedule`
- **When** llega la ejecución programada semanal
- **Then** ambos jobs (`cargo audit` y `npm audit`) se ejecutan sobre la rama por defecto

#### Scenario: Única excepción documentada

- **Given** `RUSTSEC-2026-0459` (`libsqlite3-sys`) bloqueada por sqlx
- **When** se ejecuta `cargo audit`
- **Then** esa única incidencia queda en `ignore` con justificación documentada y el resto de vulnerabilidades siguen fallando el job