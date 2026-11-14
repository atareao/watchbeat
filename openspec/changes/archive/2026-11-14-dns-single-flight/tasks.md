To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: tests de concurrencia que fallan (N consultas en lugar de 1)
- [x] GREEN: single-flight en `DnsResolver` (una consulta en vuelo por host, resultado compartido)
- [x] RED/GREEN: error compartido; acierto de caché sin consulta; hosts distintos independientes
- [x] RED/GREEN: líder cancelado — guard RAII `InflightGuard` reclama la entrada y hay handoff
- [x] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde (345)

## Cierre

- [x] `docs/dns-single-flight.md`
- [x] `openspec archive dns-single-flight`
- [x] PR a `development` con `Closes #68`
