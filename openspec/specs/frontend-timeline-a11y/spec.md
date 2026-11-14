# frontend-timeline-a11y Specification

## Purpose
Que el timeline de checks de un monitor sea comprensible sin depender del color y operable con
teclado y con lector de pantalla, de modo que la información de salud sea accesible a todos los
usuarios.

## Requirements

### Requirement: El timeline SHALL ser distinguible sin color

Cada barra del timeline SHALL transmitir su estado (`up`, `warning`, `down`, `error`,
`indeterminate`, `no_data`) mediante al menos un canal no cromático (patrón/textura o icono), de
modo que dos estados distintos sean distinguibles en escala de grises.

#### Scenario: Estados distinguibles en escala de grises
Given el timeline con barras de estados `up`, `warning`, `down`, `error` e `indeterminate`
When se renderiza sin información de color
Then cada estado se distingue de los demás por su patrón o icono
And el color no es el único canal que los diferencia

#### Scenario: Estado desconocido
Given un estado no mapeado
When se renderiza la barra
Then usa el patrón y la etiqueta de reserva (`no_data`) y no falla

### Requirement: Cada barra SHALL exponer un texto accesible

Cada barra del timeline SHALL tener un nombre accesible (`aria-label` o equivalente) que incluya
el estado y los datos del intervalo: porcentaje de UP, latencia media, número de checks y marca
temporal.

#### Scenario: Lector de pantalla anuncia el estado
Given una barra con estado `down`
When un lector de pantalla enfoca la barra
Then anuncia al menos el estado y el porcentaje de UP del intervalo

#### Scenario: Intervalo sin datos
Given una barra sin datos
When se expone su nombre accesible
Then el texto indica que no hay datos

### Requirement: El timeline SHALL ser navegable por teclado

Cada barra SHALL ser alcanzable con teclado y, al recibir el foco, SHALL mostrar su detalle
(equivalente al tooltip de hover) y un indicador de foco visible.

#### Scenario: El foco revela el detalle
Given el timeline renderizado
When el usuario navega con Tab hasta una barra
Then la barra recibe el foco
And su detalle se muestra sin necesidad de ratón

#### Scenario: Indicador de foco visible
Given una barra enfocada por teclado
When está activa
Then muestra un contorno de foco visible
