# Tests de conexión rechazada deterministas (#55)

- **Issue**: [#55](https://github.com/atareao/watchbeat/issues/55)
- **Plan**: `plans/PLAN-006-ARCHIVED.md`
- **Change OpenSpec**: `openspec/changes/archive/…-flaky-connect-refused-tests/`

## Qué es

Los tests que esperan una conexión rechazada (`connect_refused`) usan un puerto
de loopback **fuera del rango efímero** donde nadie escucha. Así el kernel
responde `ECONNREFUSED` de forma determinista, incluso con la suite en paralelo.

## Por qué

El helper anterior:

```rust
let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
listener.local_addr().unwrap().port() // listener dropped on return
```

Reservaba un puerto efímero y soltaba el listener. Bajo ejecución paralela, otro
test que también bindea `:0` podía recibir **el mismo puerto** en la ventana
entre el `drop` y el `connect`. Entonces la conexión tenía éxito y el test
fallaba (~1 de 5 ejecuciones) con `left: "up"`.

## Cómo funciona

```rust
async fn closed_loopback_port() -> u16 { 1 }
```

El puerto `1` es privilegiado y queda **fuera del rango efímero**
(`32768-60999` en Linux), así que `bind(:0)` nunca lo asigna a un test. Nadie
escucha en él → `ECONNREFUSED` inmediato.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/checker/mod.rs` | helper `closed_loopback_port` + test de determinismo |

## Cómo verificar

```bash
cd backend && cargo test --lib
```

- `closed_loopback_port_should_be_deterministic_and_refused` comprueba que el
  puerto está fuera del rango efímero y que el connect se rechaza.
- Validación manual: 20 ejecuciones completas consecutivas sin fallos.

## Decisiones

- **Puerto fijo, no aleatorio**: el objetivo es que ningún otro test pueda
  ocuparlo, no repartir puertos. El puerto 1 lo garantiza sin coordinación.
- **Sin `Mutex` global**: no hace falta serializar los tests; el puerto fijo
  elimina la carrera por construcción.
- **Sin cambios de producción**: es un helper exclusivo de `#[cfg(test)]`.

## Follow-ups

- Si algún día se ejecutan los tests como root y un servicio real ocupa el
  puerto 1, elegir otro privilegiado (p. ej. 7) o leer
  `/proc/sys/net/ipv4/ip_local_port_range` y restar uno.
