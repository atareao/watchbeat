To check an item, replace `[ ]` with `[x]`.

## Preparación
- [x] Issue #35 creado
- [x] Change proposal OpenSpec creado
- [x] Rama `feature/robust-dns-resolver`

## RED — Tests
- [x] Test unitario: resolución de host nuevo devuelve IP y la cachea
- [x] Test unitario: segunda llamada dentro del TTL no consulta upstream
- [x] Test unitario: entrada expirada se re-resuelve
- [x] Test unitario: fallback al segundo upstream si el primero no responde
- [x] Test unitario: todos los upstreams caídos → error `Timeout`
- [x] Test unitario: NXDOMAIN → error `Nxdomain`
- [x] Test unitario: SERVFAIL → error `Servfail`
- [x] Test integración: check HTTP usa la IP de caché y conserva la cabecera `Host`
      (el SNI se preserva por diseño al no reescribir la URL; no se asserta con
      handshake TLS real)
- [x] Test integración: check TCP/TLS usan la IP de caché
- [x] Test unitario: fallo DNS en TCP/TLS → `status: "error"` (no `"down"`)
- [x] Test unitario: la resolución DNS entra dentro del timeout del monitor

## GREEN — Implementación
- [x] Añadir dependencia `hickory-resolver` a `backend/Cargo.toml`
- [x] Crear `backend/src/dns.rs` (resolver + caché + upstreams + `DnsError`)
- [x] Implementar `reqwest::dns::Resolve` respaldado por la caché e integrarlo en `http_client()`
- [x] Integrar resolución propia en `TcpChecker` y `TlsChecker`
- [x] Exportar el módulo en `backend/src/lib.rs`

## REFACTOR
- [x] Eliminar resolución implícita del SO donde aplique
- [x] `cargo clippy -- -D warnings` limpio
- [x] `cargo fmt --check` limpio

## Documentación
- [x] Crear `docs/robust-dns-resolver.md`

## Cierre
- [x] `cargo test` verde
- [x] PR a `development` (`Closes #35`)
- [x] `openspec archive robust-dns-resolver`
- [x] Actualizar `plans/PLAN-002.md`
