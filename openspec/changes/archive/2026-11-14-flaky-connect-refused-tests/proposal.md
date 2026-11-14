# Proposal

**Issue:** [#55](https://github.com/atareao/watchbeat/issues/55)
**Plan:** `plans/PLAN-006.md`

## Why

El helper de test `closed_loopback_port()` reserva un puerto efímero, suelta el
listener y devuelve el puerto. Bajo ejecución paralela, otro test que también
bindea `:0` puede recibir el mismo puerto en la ventana entre el `drop` y el
`connect`; entonces la conexión tiene éxito y `tcp_check_should_report_connect_refused_cause`
(o `test_http_check_down`) falla de forma intermitente (~1 de 5 ejecuciones).

## What Changes

- `closed_loopback_port()` deja de usar puertos efímeros: devuelve un puerto
  fijo fuera del rango efímero (`1`, privilegiado, nunca asignado por `bind(:0)`
  a un proceso no-root) donde nadie escucha, de modo que el kernel rechaza el
  connect siempre.
- Se añade un test que verifica que el puerto está fuera del rango efímero y
  que la conexión se rechaza.

## Capabilities

### Added Capabilities

- `testing`: los tests que esperan `connect_refused` SHALL usar un puerto no
  efímero y determinista.

## Impact

- `backend/src/checker/mod.rs` (helper de test + un test).
- Sin cambios de producción.
