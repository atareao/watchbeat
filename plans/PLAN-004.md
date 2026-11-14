# PLAN-004 · Checker de ping: diagnóstico y permisos (#31)

- **Issue**: [#31](https://github.com/atareao/watchbeat/issues/31)
- **Rama**: `feature/ping-checker-diagnostics`
- **Change OpenSpec**: `openspec/changes/ping-checker-diagnostics/`

## Síntoma

Los monitores de tipo `ping` fallan siempre con `Ping failed (no response)` y sin
información útil. Desde dentro del contenedor, `ping` responde si se ejecuta a mano.

## Causa raíz (reproducida)

El contenedor corre como usuario **no-root** (`app`, uid 1000). El `ping` de
BusyBox (incluido en Alpine) necesita `CAP_NET_RAW`, que un proceso no-root no
tiene. Reproducido con Podman:

```
$ podman run --rm alpine:3.23 sh -c 'su nobody -s /bin/sh -c "ping -c 1 8.8.8.8"'
ping: permission denied (are you root?)
```

`docker exec` funciona porque entra como **root**. El checker ve el exit code
distinto de cero y lo traduce al genérico `Ping failed (no response)`, ocultando
el `stderr` real.

## Solución

1. **Imagen**: instalar `iputils` + `libcap` y aplicar
   `setcap cap_net_raw+ep /bin/ping`.
2. **Compose**: `cap_add: [NET_RAW]` (necesario en Podman rootless; en Docker el
   bounding set ya lo incluye, pero es inocuo).
3. **Checker**: capturar `stderr` y el código de salida; detectar
   `permission denied` / `operation not permitted` con un mensaje accionable;
   detectar binario ausente; parsear `time=` y `tiempo=` (locale).

## Tareas (TDD)

- [x] RED: tests de `parse_ping_time` (`time=`, `tiempo=`, `<1ms`, ausente).
- [x] RED: tests de `classify_ping_failure` (permisos, binario ausente, sin respuesta, stderr incluido).
- [x] GREEN: refactor de `PingChecker` con helpers puros.
- [x] GREEN: Dockerfile (`iputils`, `libcap`, `setcap`) + `compose.yml` (`cap_add`).
- [x] REFACTOR: clippy/fmt, tests verdes.
- [x] Docs: `docs/ping-checker.md`.
- [x] PR a `development` con `Closes #31`.
