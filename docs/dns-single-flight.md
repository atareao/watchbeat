# Single-flight de resolución DNS

Dedupe de consultas DNS **en vuelo** (issue #68, bloque `a` de #62). Antes, N resoluciones
concurrentes del mismo host en cache-miss lanzaban N consultas al upstream (*thundering herd*);
ahora comparten una única consulta y su resultado.

## Ficheros implicados

- `backend/src/dns.rs` — `DnsResolver` (`resolve_all`, `resolve_shared`, `join_or_lead`,
  `InflightGuard`) + tests.

## Cómo funciona

1. `resolve_all` normaliza el host y saca por la vía rápida las IP literales y los aciertos de
   caché, **sin coordinación**.
2. En cache-miss delega en `resolve_shared`:
   - **Mecanismo**: mapa `inflight: std::sync::Mutex<HashMap<String, tokio::sync::watch::Receiver<Option<Result<Vec<IpAddr>, DnsError>>>>>`
     indexado por host normalizado.
   - **Líder**: crea un `watch::channel(None)`, inserta la entrada y ejecuta `query_upstreams`.
     Publica el resultado con `tx.send(Some(..))` y elimina la entrada mediante un guard RAII.
   - **Seguidor**: clona el `Receiver`, espera `changed()` si aún no hay valor y devuelve el
     resultado publicado (mismo `Ok` o mismo `Err` que el líder).
   - **Handoff**: si el seguidor ve `None` (el líder desapareció sin publicar), re-comprueba la
     caché y puede pasar a líder en la siguiente época, acotado por `MAX_INFLIGHT_ATTEMPTS = 2`;
     agotadas las épocas resuelve directamente. Sin recursión ni bucle infinito.
   - `join_or_lead` considera **muerta** cualquier entrada con el canal cerrado
     (`has_changed().is_err()`) y la sustituye en vez de unirse a ella.
3. El guard del `std::sync::Mutex` **nunca** cruza un `.await` (la future es `Send`; lo exige
   `tokio::spawn` en los tests).

## El guard RAII (`InflightGuard`)

`resolve_all` se ejecuta bajo `tokio::time::timeout` (p. ej. en `checker/mod.rs` y
`checker/tls.rs`), así que un líder **cancelado** a mitad de `query_upstreams` es un caso real.
Sin el guard, ese líder dejaba su entrada huérfana en el mapa *para siempre*, y todas las
llamadas futuras de ese host se convertían en seguidoras de un canal muerto (dedupe rota y mapa
creciendo sin cota). `InflightGuard` implementa `Drop` y elimina la entrada del líder al salir,
tanto en retorno normal como en error, cancelación o panic. El orden *publicar → eliminar* queda
garantizado (el guard se dropea antes que el sender).

## Tests (`#[cfg(test)] mod tests`)

- `concurrent_resolutions_of_one_host_should_query_upstream_once`: 8 llamadas concurrentes →
  1 consulta, mismo resultado.
- `concurrent_resolutions_of_one_host_should_share_the_same_error`: error compartido, 1 consulta.
- `cached_host_should_not_coordinate_and_never_query_upstream`: acierto de caché → 0 consultas.
- `distinct_hosts_should_not_be_deduplicated`: dos hosts → una consulta por host.
- `cancelled_leader_is_reclaimed_and_hands_off`: líder abortado → la entrada se reclama
  (`inflight_len() == 0`) y una resolución posterior vuelve a liderar.

Todos offline con el `FakeUpstream` de test (`Behaviour::AnswerAfter` para forzar el solape).

## Troubleshooting

- Si un host deja de deduplicar, comprobar que no hay líderes bloqueados sin timeout: la
  cancelación se maneja, pero un líder que nunca completa retiene la entrada mientras vive.
- `inflight_len()` (solo test) permite inspeccionar el número de consultas en vuelo.
