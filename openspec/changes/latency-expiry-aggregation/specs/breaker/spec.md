## ADDED Requirements

### Requirement: El breaker SHALL agrupar las notificaciones de latencia y expiración

El breaker SHALL agrupar las notificaciones de tipo `latency` y `expiry` con la misma
ventana `Y` (`storm_window_seconds`) y el mismo umbral
`max(min_monitors, ceil(X % · monitores_habilitados))` que usa para las caídas,
contabilizando **monitores distintos** que emiten la misma señal dentro de la ventana. Al
alcanzar el umbral, el breaker SHALL devolver una `BreakerDecision::OpenAggregate` con el
recuento y la lista de monitores afectados, y SHALL emitirse **una única** alerta agregada
de ese tipo (plantilla por defecto `STORM_LATENCY`/`STORM_EXPIRY`) a la unión de los
notificadores de los monitores afectados. Mientras el episodio esté abierto, las señales
siguientes de ese mismo tipo SHALL suprimirse (`BreakerDecision::Suppress`). El tipo de
señal SHALL modelarse con `SignalKind { Latency, Expiry }` y evaluarse mediante
`StormBreaker::on_signal(now, kind, monitor_id, monitor_name, enabled_monitors)`. NO SHALL
requerir cambios de esquema.

#### Scenario: Ráfaga de latencia suprimida por una única agregada
- **Given** 8 monitores habilitados, umbral 50 % y una ventana de 10 s
- **When** 4 monitores distintos brechan el umbral de latencia dentro de la ventana
- **Then** se emite una única alerta agregada de latencia con sus 4 monitores
- **And** las señales de latencia siguientes dentro del episodio se suprimen

#### Scenario: Notificación tras la ventana
- **Given** un episodio de latencia abierto cuyo último aviso ocurrió hace más de la ventana
- **When** vuelven a brechar el umbral suficientes monitores dentro de una nueva ventana
- **Then** se vuelve a agrupar y se emite una nueva alerta agregada de latencia

#### Scenario: Un monitor que oscila no declara una tormenta de señal
- **Given** un único monitor que emite señales de latencia repetidas dentro de la ventana
- **When** todas las señales provienen del mismo monitor
- **Then** NO se declara tormenta y cada señal pasa individualmente

### Requirement: Los episodios de señal SHALL ser independientes por tipo

El breaker SHALL mantener episodios separados para las caídas (`down`/`up`), para `latency`
y para `expiry`. Abrir o suprimir un tipo NO SHALL afectar al estado ni al despacho de los
otros tipos. El rearme de cada episodio SHALL ser perezoso: transcurrida la ventana `Y` sin
nuevas señales de ese tipo, el episodio vuelve a `Closed`.

#### Scenario: Latencia abierta no suprime expiración
- **Given** una tormenta de latencia abierta y su alerta agregada emitida
- **When** varios monitores emiten señales de expiración dentro de la ventana
- **Then** las señales de expiración se agrupan/notifican con independencia de la latencia

#### Scenario: Latencia abierta no suprime caídas
- **Given** una tormenta de latencia abierta
- **When** un monitor cae (`down`)
- **Then** la lógica de caídas se evalúa con su propio episodio y su propia decisión

### Requirement: El breaker desactivado NO SHALL agrupar señales

Cuando `storm_breaker_enabled = false`, el breaker NO SHALL agregar ni suprimir las señales
de `latency`/`expiry`: cada señal SHALL devolver `BreakerDecision::Pass` (comportamiento
clásico), igual que para las caídas.

#### Scenario: Desactivado no agrupa latencia
- **Given** `storm_breaker_enabled = false` y una ráfaga de señales de latencia
- **When** varios monitores brechan el umbral de latencia dentro de la ventana
- **Then** cada señal se notifica individualmente
- **And** no se emite alerta agregada
