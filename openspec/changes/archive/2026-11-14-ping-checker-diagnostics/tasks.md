To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: tests de `parse_ping_time` (`time=`, `tiempo=`, `<1ms`, ausente)
- [x] RED: tests de `classify_ping_failure` (permisos, binario ausente, sin respuesta, stderr)
- [x] GREEN: refactor de `PingChecker` con helpers puros
- [x] GREEN: Dockerfile (`iputils`, `libcap`, `setcap`) + `compose.yml` (`cap_add`)
- [x] REFACTOR: clippy/fmt limpios, tests verdes

## Cierre

- [x] `docs/ping-checker.md`
- [x] `openspec archive ping-checker-diagnostics`
- [x] PR a `development` con `Closes #31`
