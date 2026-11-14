To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: tests de `JwtValidator` (válido, `aud`, `iss`, `exp`, algoritmo, `kid`) que fallan
- [x] GREEN: costura de test offline + mínima implementación para que pasen
- [x] RED: `security.yml` con jobs de auditoría (verificación de fallo ante vulnerabilidad simulada)
- [x] GREEN: `security.yml` verde con audits reales + `ignore` documentado de RUSTSEC-2026-0459
- [x] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde

## Cierre

- [x] `docs/jwt-validation-tests.md` y `docs/security-ci.md`
- [x] `openspec archive jwt-tests-security-ci`
- [x] PR a `development` con `Closes #61`