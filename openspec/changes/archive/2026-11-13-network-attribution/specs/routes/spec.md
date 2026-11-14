# Routes — Página de estado pública

## ADDED Requirements

### Requirement: La página de estado pública SHALL distinguir indeterminate

La página de estado pública SHALL representar el estado `indeterminate` de forma
distinta de `down` (no como una caída del servicio) y NO SHALL contabilizarlo como
monitor caído en el estado global de la página.

#### Scenario: Monitor indeterminate no se muestra como caída
Given una status page con un monitor en estado `indeterminate`
When se renderiza la página pública
Then el monitor NO se muestra como `down`
And se representa con un estado propio (`indeterminate`)

#### Scenario: La etiqueta visible respeta el idioma de la página
Given una status page con un monitor en estado `indeterminate`
When se renderiza la página pública
Then la etiqueta visible está en el idioma de la página (español)
And el valor interno del estado NO se altera

#### Scenario: El estado global no cuenta indeterminate como caída
Given una status page con monitores `up` e `indeterminate`
When se calcula el estado global de la página
Then el estado global NO es «caído»
