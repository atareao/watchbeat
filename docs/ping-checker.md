# Checker de ping: diagnóstico y permisos (#31)

- **Issue**: [#31](https://github.com/atareao/watchbeat/issues/31)
- **Plan**: `plans/PLAN-004.md`
- **Change OpenSpec**: `openspec/changes/archive/…-ping-checker-diagnostics/`

## Qué es

El checker de ping ejecuta `ping -c 1 -W <timeout> -- <target>` y traduce la salida
a un `CheckOutcome`. Ahora **preserva el `stderr` y el código de salida** y
distingue tres familias de fallo:

| Familia | Status | Mensaje |
|---|---|---|
| Target no responde | `down` | `Ping failed: <stderr> (exit code N)` |
| Sin permisos (`CAP_NET_RAW`) | `error` | referencia explícita a `CAP_NET_RAW` y `cap_add` |
| Binario ausente | `error` | indica instalar `iputils` |

## Por qué

Los monitores de tipo ping fallaban siempre con un genérico
`Ping failed (no response)`. La causa raíz: el contenedor corre como usuario
**no-root** (`app`, uid 1000) y el `ping` de BusyBox necesita `CAP_NET_RAW`. El
checker solo miraba el exit code y ocultaba el `stderr` real (`ping: permission
denied (are you root?)`). `docker exec` funcionaba porque entra como root.

## Cómo funciona

1. El checker captura `out.stderr` y `out.status.code()` y los pasa a
   `classify_ping_failure`.
2. `classify_ping_failure` inspecciona el texto en minúsculas:
   - `permission denied` / `operation not permitted` → `error` + guía de
     `CAP_NET_RAW`.
   - exit code `127` / `command not found` → `error` + guía de `iputils`.
   - cualquier otro caso → `down` con el `stderr` y el código de salida.
3. Si el binario no se puede lanzar (`Command::output` falla), el error de
   spawn se clasifica igual (sin código de salida).
4. El target se pasa tras `--`, de modo que un valor que empiece por `-` nunca
   se interpreta como flag de `ping`.
5. En el camino `up`, `parse_ping_time` extrae el RTT de `time=` o `tiempo=`
   (locale) y trata `time<1 ms` como `0`.

## Imagen y compose

| Cambio | Fichero |
|---|---|
| `apk add iputils libcap` + `setcap cap_net_raw+ep` al `ping` | `Dockerfile` |
| `cap_add: [NET_RAW]` | `compose.yml` |

`setcap` basta para ejecutar `ping` como usuario no-root. `cap_add: [NET_RAW]`
es defensa en profundidad: garantiza la capacidad aunque el filesystem pierda
los xattrs. En Podman rootless el comportamiento depende del backend de red
(pasta/slirp4netns), así que `NET_RAW` no siempre sustituye al `setcap`.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/checker/mod.rs` | `parse_ping_time`, `classify_ping_failure`, `PingChecker` |
| `Dockerfile` | `iputils`, `libcap`, `setcap` |
| `compose.yml` | `cap_add: [NET_RAW]` (y entrecomillado de `OIDC_REDIRECT_URL`, fix YAML) |

El `Dockerfile` elimina el bit setuid de `ping` (`chmod u-s`) antes de aplicar
`setcap`, para que el binario no sea setuid-root dentro del contenedor.

## Cómo verificar

```bash
cd backend && cargo test --lib ping
```

Tests deterministas de los helpers puros:

1. `parse_ping_time` con `time=` y `tiempo=`, sub-milisegundo y ausencia.
2. `classify_ping_failure`: permisos, `operation not permitted`, binario ausente,
   sin respuesta y con `stderr`/exit code.

En un contenedor:

```bash
podman run --rm -u app --cap-add NET_RAW <imagen> ping -c 1 8.8.8.8
```

## Decisiones

- **Diagnóstico, no reintentos**: el issue era visibilidad, no un falso positivo
  por repetición. Un solo ping, una sola decisión.
- **Helpers puros** (`parse_ping_time`, `classify_ping_failure`): testeables sin
  lanzar procesos ni requerir red.
- **`down` vs `error`**: un target que no responde es un fallo del target
  (`down`); una configuración rota (permisos, binario) es `error` y no debe
  contar como caída del objetivo.

## Follow-ups

- Monitorizar en arranque que `ping` tenga `cap_net_raw` y avisar en logs si no.
- Añadir tests de integración que ejecuten `ping` real contra `127.0.0.1`.
