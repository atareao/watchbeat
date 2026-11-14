## MODIFIED Requirements

### Requirement: El timeline SHALL ser navegable por teclado

El timeline SHALL exponer **una única parada de tabulación por gráfico** (roving tabindex): solo la
barra activa SHALL ser alcanzable con `Tab`; las flechas ←/→ (y Home/End) SHALL mover la barra
activa dentro del gráfico; al recibir el foco, la barra activa SHALL mostrar su detalle
(equivalente al tooltip de hover) y un indicador de foco visible.

#### Scenario: Una sola parada de Tab por gráfico
Given un gráfico con N barras
When el usuario tabula hacia el gráfico
Then solo la barra activa recibe el foco
And las demás barras no son paradas de Tab

#### Scenario: Las flechas mueven el foco
Given el gráfico enfocado en una barra activa
When el usuario pulsa ← o →
Then la barra activa pasa a la anterior o a la siguiente
And Home/End mueven a la primera o a la última barra

#### Scenario: El foco revela el detalle
Given el timeline renderizado
When el usuario navega con Tab hasta una barra
Then la barra recibe el foco
And su detalle se muestra sin necesidad de ratón

#### Scenario: Indicador de foco visible
Given una barra enfocada por teclado
When está activa
Then muestra un contorno de foco visible
