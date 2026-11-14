# Timeline a11y + test SNI real

Cierra el issue [#63](https://github.com/atareao/watchbeat/issues/63) (follow-up de #62).
Change OpenSpec: `openspec/changes/timeline-a11y-sni-test/` (capabilities `frontend-timeline-a11y`,
`checker`, `testing`).

## Qué es

Dos bloques independientes que comparten issue:

1. **Timeline de checks accesible**: las barras del timeline dejaban de ser legibles sin percibir
   color. Ahora cada estado se distingue además por un canal **no cromático** (patrón + borde
   superior) y expone un **texto accesible** por barra; el timeline es **navegable por teclado**.
2. **Test SNI determinista**: se blinda por test que el checker TLS envía el **hostname del
   monitor como SNI**, sin red externa.

## Ficheros implicados

| Fichero | Cambio |
|---|---|
| `frontend/src/theme/statusStyles.ts` | **NUEVO** — token único `estado → { color, pattern, borderTop, label, icon }` + `statusStyle()` con fallback `no_data` |
| `frontend/src/pages/MonitorDetail.tsx` | patrón + `borderTop` + `role="img"` + `tabIndex` + `aria-label` en las barras; `StatusPatternLegend()`; `healthColor()` consume el token |
| `frontend/src/components/MonitorCard.tsx` | reutiliza el token (`CARD_STATUS` → `statusStyle`), iconos decorativos |
| `frontend/src/pages/Dashboard.tsx` | colores hardcodeados → token |
| `frontend/src/global.css` | `.timeline-bar:focus-visible` (anillo de foco `outline-offset:-2px`) |
| `backend/src/checker/tls.rs` | refactor `attempt`→`attempt_with` (connect→connector→handshake en un único sitio) + 2 tests SNI |
| `backend/Cargo.toml` / `Cargo.lock` | `rcgen = "0.14"` en `[dev-dependencies]` (no entra en el binario) |

## Cómo funciona

### Frontend — canal no cromático

El token define por estado un `pattern` (`repeating-linear-gradient` con ángulo/periodo distinto) y
un `borderTop` (grosor + estilo distinto). El `borderTop` es el canal que sobrevive a barras
pequeñas, donde la trama no se aprecia:

| estado | pattern | borderTop |
|---|---|---|
| `up` | sólido | — |
| `warning` | 45° | `2px solid` |
| `down` | 0° | `3px solid` |
| `error` | 90° | `3px double` |
| `indeterminate` | 135° (claro) | `2px dotted` |
| `no_data` | −45° disperso | `1px dashed` |

Cada barra: `role="img"`, `tabIndex={0}`, `aria-label` (estado + %UP + latencia + nº checks +
hora; en `indeterminate`/`no_data` se omiten up%/latencia por no ser atribuibles) y
`Tooltip trigger={["hover","focus"]}`. La leyenda `StatusPatternLegend()` explica el canal con
`StatusStyle.label`.

### Backend — SNI

`attempt_with(host, ip, port, addr, make_connector)` es el único sitio con el orden
`connect → build_connector → handshake`: la factory del connector se llama **después** del TCP OK,
así una conexión rechazada no necesita crypto provider. Producción (`attempt`) y el seam de test
(`check_with_all_with_connector`, `#[cfg(test)]`) comparten ese camino. El test levanta un servidor
`tokio-rustls` en loopback con un cert `rcgen` para `example.test`, captura `client_hello.server_name()`
por canal (sin sleeps) y asserta SNI == `example.test`; un segundo test verifica que un target IP
literal **no** envía SNI (RFC 6066).

## Variables / configuración

Ninguna. Sin cambios de API, esquema ni migraciones.

## Cómo verificar

```bash
cd backend && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
cd frontend && npx tsc --noEmit && npm run build
openspec validate timeline-a11y-sni-test --strict
```

## Decisiones

- **Patrón CSS + `borderTop`** en vez de overlay SVG por barra: coste ~0 y funciona en escala de grises.
- **Token único** `statusStyles.ts`: elimina la duplicación de colores de estado en 3 ficheros.
- **Sin test runner de frontend**: el issue pide `tsc` + build; no se introduce vitest (fuera de alcance).
- **`rcgen` como dev-dependency**: el test SNI es 100% en proceso; no añade peso al binario.
- **`error` `#f59e0b` → `#f97316`**: separa `error` (naranja) de `warning` (ámbar), antes compartido.

## Follow-ups

- Contraste AA de los `Tag` de `MonitorCard` (texto blanco sobre `#f97316`/`#22c55e`) — pre-existente.
- Roving tabindex en el timeline (hoy 1 parada de tab por barra).
- Migrar `statusStyles.ts` a `.tsx` (JSX) si se unifica con otros tokens.
