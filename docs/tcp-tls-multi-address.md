# Fallback multi-dirección en TCP y TLS (#57)

- **Issue**: [#57](https://github.com/atareao/watchbeat/issues/57)
- **Plan**: `plans/PLAN-007-ARCHIVED.md`
- **Change OpenSpec**: `openspec/changes/archive/…-tcp-tls-multi-address-fallback/`

## Qué es

Los checkers `tcp` y `tls` resuelven **todas** las direcciones del host
(`resolve_all`) y las intentan en orden hasta que una conecta (TLS: hasta
completar el handshake). Un registro A/AAAA obsoleto ya no provoca un falso
`down`.

## Por qué

#52 corrigió este patrón para HTTP, pero `TcpChecker` y `TlsChecker` seguían
usando `dns::resolve` (primera IP). Con varios registros y uno muerto, el
monitor podía caer aunque otra dirección estuviera viva.

## Cómo funciona

1. `resolve_all(host)` devuelve todas las direcciones (deduplicadas, IPv4
   primero, cache TTL).
2. Se itera por cada dirección compartiendo el presupuesto de timeout del
   monitor: `remaining = timeout - elapsed`.
3. TCP: éxito en el primer `connect` que funcione. TLS: éxito en el primer
   handshake completo.
4. Si **todas** fallan, se conserva la causa del primer intento y el mensaje
   resume cada dirección: `TCP connection failed for all addresses: ip1: err1; ...`.

La resolución y la conexión se cronometran por separado, así que un timeout de
DNS (`DnsTimeout`) sigue distinguiéndose de uno de conexión (`ConnectTimeout`).

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `backend/src/dns.rs` | `resolve_all(host)` libre sobre el resolver global |
| `backend/src/checker/mod.rs` | `TcpChecker::check_all` + bucle multi-dirección |
| `backend/src/checker/tls.rs` | `TlsChecker::check_all` + `attempt` inyectable |

## Cómo verificar

```bash
cd backend && cargo test --lib checker
```

Tests deterministas (sin red externa, con resolver y conector inyectados):

1. TCP con dos IPs (una rechaza, una acepta) → `up`.
2. TLS con dos IPs (una rechaza, una completa handshake simulado) → `up`.
3. Todas las direcciones rechazan → `down` + `connect_refused` + resumen.
4. Host con una sola dirección → comportamiento idéntico al anterior.

## Decisiones

- **Primera causa, no última**: el resultado es determinista; el mensaje incluye
  todos los intentos para diagnóstico.
- **Métodos inyectables**: `check_with`/`check_with_connect` quedan como
  `#[cfg(test)]` para los tests de una sola dirección; producción usa `check_all`.
- **Timeout compartido**: las direcciones no reinician el presupuesto; un host
  con muchas IPs no puede alargar el check indefinidamente.
- **Connector TLS tras el connect**: el `TlsConnector` (roots + config) se
  construye solo cuando el TCP connect ha tenido éxito. Así un fallo de conexión
  no exige crypto provider instalado (los tests de `connect_refused` no
  paniquean). Si varias IPs aceptan TCP pero fallan el handshake, cada intento
  reconstruye el connector (caso raro; el coste es bajo comparado con el I/O).
- **Tests sobre la ruta real**: además de los injectables, se cubre `check_all`
  con un listener TCP real (up), una IP sin listener (down/refused) y un servidor
  TCP plano (handshake → `tls_error`).

## Follow-ups

- Acotar el número de direcciones intentadas (p. ej. 8) para hosts con muchos
  registros.
- Registrar en log/métrica cuándo el fallback salva el check (DNS sucio).
