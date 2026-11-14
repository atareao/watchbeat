# PLAN-006 · Fix test flaky de conexión rechazada (#55)

- **Issue**: [#55](https://github.com/atareao/watchbeat/issues/55)
- **Rama**: `feature/flaky-connect-refused-tests`
- **Change OpenSpec**: `openspec/changes/archive/2026-11-14-flaky-connect-refused-tests/`

## Causa

`closed_loopback_port()` bindea `127.0.0.1:0`, obtiene el puerto y suelta el
listener. Bajo paralelismo, otro test puede recibir el mismo puerto efímero
antes del `connect`, así que la conexión tiene éxito (`up`) en vez de rechazarse.

## Solución

Devolver un puerto fijo **fuera del rango efímero** (`1`), nunca asignado por
`bind(:0)` a un proceso no-root. Nadie escucha ahí → `ECONNREFUSED` siempre.

## Tareas (TDD)

- [x] RED: test puerto no efímero + conexión rechazada.
- [x] GREEN: `closed_loopback_port` determinista.
- [x] REFACTOR: clippy/fmt + 20 ejecuciones sin flaky.
- [x] Docs, archive, PR con `Closes #55`.
