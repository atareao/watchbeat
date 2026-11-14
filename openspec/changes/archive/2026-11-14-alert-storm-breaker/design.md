# Design

## Context

Ver `proposal.md` — Why. Punto de integración real (`scheduler.rs:488-560`): se calcula
`notification_type: Option<(&str, String)>` (transiciones `down`/`up`, `expiry`, `latency`)
y justo después se despacha a los notificadores por monitor (líneas 560-775). El bucle de
gestión (`manager_loop`, 123-209) ya mantiene caches compartidas (`notifier_cache`) y recibe
`SchedulerCommand`s; el canario `NetworkHealth` está disponible vía `health.current()`.

## Goals / Non-Goals

**Goals**
- Una incidencia que tumba a varios monitores a la vez produce **como máximo una** alerta.
- Detección determinista y **testeable sin sleeps** (reloj inyectado).
- Configurable y desactivable; comportamiento por defecto seguro.
- Reutilizar el patrón existente: settings KV + template + comando de recarga en caliente.

**Non-Goals**
- Reintentos/debounce (rechazados por diseño; la atribución es la Fase 3).
- Sustituir al canario de red (Fase 3): el breaker gobierna el **volumen** de alertas,
  no la atribución.
- Agrupar `latency`/`expiry` (solo `down`/`up` participan de la tormenta).
- Exponer el estado del breaker en la API/UI (queda como follow-up).

## Decisions

- **D1 · Máquina de estados pura con reloj inyectado.** `StormBreaker` en
  `backend/src/breaker.rs` recibe `now: Instant` en cada llamada (`on_down`, `on_up`) y
  mantiene los eventos de caída en un `VecDeque<(Instant, monitor)>`. Así los tests
  avanzan el reloj a mano y **no usan `sleep` real** (mismo principio que la Fase 4).
- **D2 · Estados `Closed` / `Open`.** `Closed` = pasa todo. `Open` = tormenta en curso:
  se suprime lo individual y ya se emitió la agregada. La transición `Open → Closed` es
  **perezosa**: se evalúa en la siguiente llamada cuando `now − última_caída ≥ ventana`.
- **D3 · Umbral.** `threshold = max(min_monitors, ceil(X % · monitores_habilitados))`,
  con `min_monitors` por defecto 3. Si `monitores_habilitados < min_monitors`, el breaker
  **nunca** dispara (despliegues pequeños conservan la semántica clásica de 1 alerta por
  monitor). Decisión: umbral **absoluto mínimo** para no agregar una sola caída.
- **D4 · Dedupe por episodio.** Durante `Open` no se repite la agregada aunque sigan
  cayendo monitores; la lista de afectados se refresca pero no se reenvía. Solo al rearmarse
  y volver a superar el umbral se emite una nueva agregada → **una alerta por tormenta**.
- **D5 · Supresión `down` y `up` durante la tormenta.** Se suprimen ambas transiciones
  mientras esté `Open` (una recuperación a media tormenta también es ruido). La alerta
  agregada al abrir ya informa del suceso; las recuperaciones individuales posteriores
  vuelven a pasar cuando el breaker ya está `Closed`.
- **D6 · Configuración por settings (KV) + recarga en caliente.**
  - Claves: `storm_breaker_enabled` (`true`), `storm_threshold_percent` (`50`),
    `storm_window_seconds` (`10`), `storm_min_monitors` (`3`), `default_template_storm`.
  - `BreakerConfig::load(&db)` lee las claves con esos defaults.
  - Nuevo `SchedulerCommand::ReloadBreaker`: la ruta `set_setting` lo emite cuando la clave
    empieza por `storm_` o es `default_template_storm`, y `manager_loop` reconfigura el
    breaker. Sin esto, guardar en la UI no surtiría efecto hasta reiniciar.
- **D7 · Mensaje agregado con plantilla.** Al abrir, se renderiza `default_template_storm`
  (settings) con fallback a `template::defaults::STORM` (constante nueva). Contexto:
  `{ count, monitors: [nombres], monitors_list: "a, b, c", window_seconds, threshold_percent }`.
  Se envía **una vez** a la **unión deduplicada** de los notificadores de **todos** los
  monitores afectados (los `monitor_ids` del `AggregateAlert`), más el `notifier_id` del
  monitor que abre. Si la unión queda vacía, se registra un `warn` y no se envía nada (la
  tormenta sigue deduplicada), evitando suprimir a ciegas.
- **D8 · Estado compartido.** `Arc<Mutex<StormBreaker>>` (tokio) creado en `manager_loop` y
  clonado a cada `monitor_task`/`MonitorCycle`, igual que `notifier_cache`. Se bloquea de
  forma **síncrona y breve** (sin `await` bajo lock).
- **D9 · Ortogonalidad.** No toca `run_monitor_check` salvo el punto de decisión del
  despacho; `indeterminate`, canario, atribución y persistencia quedan intactos.

## Risks / Trade-offs

- [El lock compartido en el camino caliente] → bloqueo síncrono brevísimo (solo
  `VecDeque` + comparaciones); sin `await` dentro del lock. Impacto despreciable.
- [La agregada se envía a la unión de notificadores de los afectados] → resuelto en el
  fix de auditoría (MAYOR-2). Si ningún afectado tiene notificadores, se avisa por log y
  no se envía, en vez de suprimir en silencio.
- [Umbral demasiado bajo con pocos monitores] → `min_monitors` (default 3) lo evita.
- [Config mal puesta (X=0)] → `X` se interpreta como porcentaje; con umbral absoluto
  `min_monitors` sigue habiendo suelo. `enabled=false` desactiva el breaker por completo.

## Migration Plan

- Sin migración de esquema. Despliegue normal; rollback = revertir el PR.
- Comportamiento por defecto: breaker **activo** con X=50 %, Y=10 s, min=3. Un
  despliegue con < 3 monitores no cambia de comportamiento.

## Open Questions

- ¿Aplicar también la agregada a `status pages` públicas? Fuera de alcance de esta fase.
- ¿Persistir el estado del breaker entre reinicios? No: se rearma limpio al arrancar
  (una tormenta a caballo de un reinicio se re-evalúa).
