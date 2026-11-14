# ADR-0005 · Breaker con dedupe por episodio

- **Estado**: aceptado
- **Fecha**: 2026-11-14
- **Contexto**: PLAN-002, Fase 5

## Contexto

Aun con atribución (Fase 3), cuando el canario está sano pero varios targets fallan a la
vez (p. ej. un proveedor DNS del que dependen varios servicios), se enviaban N alertas por
un único suceso.

## Decisión

Un **breaker** (`StormBreaker`) que detecta tormentas por umbral (`X %` de monitores
habilitados en `Y` segundos, con suelo `min_monitors`), emite **una sola alerta agregada**
y aplica **dedupe por episodio**: mientras está `Open` no repite la agregada; se rearma al
pasar la ventana sin nuevas caídas.

## Razones

- **Una alerta por tormenta**, no una por monitor: el operador ve el suceso, no el ruido.
- **Dedupe por episodio** (no por ventana deslizante) evita reenviar la agregada mientras
  la tormenta sigue activa.
- **Dedupe por monitor único** (no por transición): un monitor que flapea no dispara una
  falsa tormenta.
- **Suelo `min_monitors`**: no se agrega una sola caída; despliegues pequeños conservan la
  semántica clásica.
- **Unión de notificadores** de todos los afectados: la agregada no se pierde si el monitor
  que abre no tiene notificadores.
- Máquina de estados **pura con reloj inyectado** → testeable sin sleeps.

## Consecuencias

- Durante `Open` se suprimen las transiciones individuales `down` **y** `up`.
- Configurable y desactivable desde la UI (settings KV + recarga en caliente).
- El estado del breaker no se persiste (arranca limpio).
