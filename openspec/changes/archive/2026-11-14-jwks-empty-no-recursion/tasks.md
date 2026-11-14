To check an item, replace `[ ]` with `[x]`.

## TDD

- [x] RED: test del servidor HTTP local `{"keys": []}` → `Err` + exactamente 1 petición (falla: stack overflow)
- [x] GREEN: eliminar la recursión de `validate_token`; fetch único + `Err` si la caché queda vacía
- [x] RED/GREEN: fallo de red propaga `Err`; caché vacía con refresco deshabilitado → `Err` sin red (con timeout)
- [x] REFACTOR: `cargo clippy -- -D warnings`, `cargo fmt --check`, suite completa verde (340)

## Cierre

- [x] `docs/jwks-empty-no-recursion.md`
- [x] `openspec archive jwks-empty-no-recursion`
- [x] PR a `development` con `Closes #64`
