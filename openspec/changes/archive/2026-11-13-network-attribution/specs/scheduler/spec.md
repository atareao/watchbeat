# Scheduler — Atribución en el momento del check

## ADDED Requirements

### Requirement: La atribución SHALL aplicarse en el momento del check

El scheduler SHALL, al evaluar cada check, clasificar el resultado combinando la
causa (`FailureCause`) con la salud de red publicada por el canario, y SHALL
persistir y notificar el estado **atribuido** (no el status base). El mismo
estado atribuido SHALL aplicarse en el `run-check` manual.

#### Scenario: Caída real con red sana se registra como down
Given un monitor que falla con causa `connect_refused`
And salud de red `healthy`
When se ejecuta el check
Then el check se persiste con status `down`

#### Scenario: Fallo por DNS degradado se registra como indeterminate
Given un monitor que falla con causa `dns_timeout`
And salud de red `dns_degraded`
When se ejecuta el check
Then el check se persiste con status `indeterminate`
And la causa `error_kind` sigue registrándose

#### Scenario: El run-check manual usa la misma atribución
Given salud de red `down`
When se lanza el `run-check` manual de un monitor que falla
Then el resultado devuelto refleja el estado atribuido (`indeterminate`)

### Requirement: El estado indeterminate NO SHALL amplificar registro ni eventos

Un periodo sostenido de `indeterminate` NO SHALL emitir un evento SSE en cada
check ni SHALL forzar una escritura por cada check: la transición a `indeterminate`
SHALL tratarse como un cambio una sola vez, y los checks siguientes SHALL seguir
la lógica de muestreo habitual.

#### Scenario: Corte sostenido no emite un evento por check
Given un monitor que pasa a `indeterminate` y permanece así varios ciclos
When se completan varios checks consecutivos `indeterminate`
Then solo el primero (transición) emite un evento SSE
And los siguientes no emiten evento salvo por muestreo

#### Scenario: Una escritura fallida no pierde la transición
Given un monitor cuya transición de estado no se pudo persistir
When se completa el siguiente check con el mismo estado
Then la transición se reintenta y se persiste
And el estado observado NO SHALL adelantarse al resultado de la escritura

## MODIFIED Requirements

### Requirement: Notifications SHALL dispatch on status transitions

Notifications SHALL be dispatched on status transitions (up→down, down→up), latency threshold breaches, and TLS certificate expiry. The scheduler SHALL use cached `notifier_ids` from DB (loaded once at task start) and SHALL fall back to `monitor.notifier_id` if the cached list is empty. El estado `indeterminate` SHALL ser **neutral**: NO SHALL disparar notificación de caída, NO SHALL cambiar el estado de notificación del monitor, y NO SHALL contar como recuperación.

#### Scenario: Down notification on status change
- **Given** a monitor in UP state
- **When** a check transitions to DOWN
- **Then** a notification with type `"down"` is dispatched

#### Scenario: Indeterminate no dispara notificación de caída
Given un monitor en estado UP
When un check se clasifica como `indeterminate`
Then NO se envía notificación de caída
And el estado de notificación del monitor permanece UP

#### Scenario: Fallo real con red sana sí notifica
Given un monitor en estado UP
And salud de red `healthy`
When un check se clasifica como `down`
Then se envía notificación de caída

