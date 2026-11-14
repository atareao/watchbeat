# Attribution — Clasificación determinista del fallo

## ADDED Requirements

### Requirement: Clasificación determinista de la atribución

El sistema SHALL decidir de forma determinista a quién pertenece un fallo —
al **target** o a la **conectividad de salida** — combinando la causa del fallo
(`FailureCause`) y la salud de red (`NetworkHealth`), mediante una función pura
sin efectos secundarios. El resultado SHALL ser uno de `up`, `down`, `error` o
`indeterminate`.

#### Scenario: Check correcto
Given un check que termina correctamente
When se clasifica el resultado
Then el estado es `up`

#### Scenario: Dominio inexistente siempre es del target
Given un fallo con causa `dns_nxdomain`
When se clasifica con cualquier salud de red
Then el estado es `down`

#### Scenario: Fallo con salida sana se atribuye al target
Given un fallo con salud de red `healthy`
When se clasifica el resultado
Then el estado es el status base de la causa (`down` o `error`)

#### Scenario: Fallo sin información de red usa el status base
Given un fallo con salud de red `unknown`
When se clasifica el resultado
Then el estado es el status base de la causa

#### Scenario: Fallo de DNS con DNS degradado es indeterminado
Given un fallo con causa `dns_timeout` o `dns_servfail`
And salud de red `dns_degraded`
When se clasifica el resultado
Then el estado es `indeterminate`

#### Scenario: Fallo no-DNS con DNS degradado se atribuye al target
Given un fallo cuya causa NO es de la familia `dns_*` (salvo `dns_nxdomain`)
And salud de red `dns_degraded`
When se clasifica el resultado
Then el estado es el status base de la causa

#### Scenario: Caída de red deja el fallo sin atribuir
Given un fallo cuya causa NO es `dns_nxdomain`
And salud de red `down`
When se clasifica el resultado
Then el estado es `indeterminate`

#### Scenario: La tabla D5 es la única fuente del veredicto
Given un fallo con causa
When se atribuye el resultado
Then el veredicto es el de la tabla D5 para esa causa y esa salud de red
And la atribución NO mantiene una copia propia de la tabla

### Requirement: El estado indeterminate no es una caída atribuible

`indeterminate` SHALL representar un fallo que NO puede atribuirse al target.
SHALL ser distinto de `down` y `error` a todos los efectos (notificación,
uptime y UI).

#### Scenario: Indeterminate no es down ni error
Given un resultado clasificado como `indeterminate`
Then no es `down`
And no es `error`

### Requirement: Los fallos sin causa también se atribuyen

Un fallo cuyo checker no asigna causa (`cause = None`) SHALL tratarse como fallo
a efectos de atribución: con salud de red `down` SHALL ser `indeterminate`; con
salud `healthy` o `unknown` SHALL conservar su status base. Un resultado correcto
sin causa NO SHALL alterarse.

#### Scenario: Fallo sin causa con la red caída es indeterminado
Given un fallo cuyo checker no asigna causa (p. ej. `ping`)
And salud de red `down`
When se clasifica el resultado
Then el estado es `indeterminate`

#### Scenario: Fallo sin causa con red sana se atribuye al target
Given un fallo cuyo checker no asigna causa
And salud de red `healthy`
When se clasifica el resultado
Then el estado es el status base del checker (`down`)

#### Scenario: Éxito sin causa no se altera
Given un resultado correcto sin causa (p. ej. `warning` de certificado)
When se clasifica el resultado
Then el estado se conserva sin cambios
