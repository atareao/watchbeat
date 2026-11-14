# Proposal

**Issue:** [#31](https://github.com/atareao/watchbeat/issues/31)
**Plan:** `plans/PLAN-004.md`

## Why

Los monitores `ping` fallan siempre con `Ping failed (no response)` y sin
diagnóstico. Causa raíz reproducida: el contenedor corre como usuario no-root y
el `ping` de BusyBox necesita `CAP_NET_RAW`; el checker oculta el `stderr` real
(`ping: permission denied (are you root?)`) tras un mensaje genérico.

## What Changes

- **`PingChecker`**: captura `stderr` y código de salida; clasifica el fallo
  (permisos, binario ausente, sin respuesta) con mensajes accionables; parsea
  `time=` y `tiempo=`.
- **Dockerfile**: instala `iputils` + `libcap` y aplica
  `setcap cap_net_raw+ep /bin/ping`.
- **`compose.yml`**: `cap_add: [NET_RAW]`.
- **Docs**: `docs/ping-checker.md` con el diagnóstico y el fix.

## Capabilities

### New Capabilities

- `deployment`: el contenedor SHALL permitir ICMP a un usuario no-root.

### Modified Capabilities

- `checker`: el checker de ping SHALL reportar la causa del fallo.

## Impact

- `backend/src/checker/mod.rs` (PingChecker + helpers puros).
- `Dockerfile`, `compose.yml`.
- Sin cambios de API ni de esquema.
