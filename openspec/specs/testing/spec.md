# testing Specification

## Purpose

Mantener deterministas los tests de red: los que esperan una conexión rechazada
no deben depender de puertos efímeros que otros tests puedan reocupar.

## Requirements

### Requirement: Test de conexión rechazada determinista

Los tests que esperan una conexión rechazada SHALL usar un puerto que no pueda
ser asignado por otro `bind(127.0.0.1:0)` concurrente, de modo que ningún otro
test pueda quedar escuchando en él. El resultado SHALL ser estable bajo
ejecución paralela.

#### Scenario: El puerto de conexión rechazada está fuera del rango efímero
Given el helper de tests `closed_loopback_port`
When se obtiene el puerto
Then el puerto es menor que el límite inferior del rango efímero
And una conexión a ese puerto se rechaza con `ECONNREFUSED`

#### Scenario: Ejecución completa repetida
Given los tests del checker
When se ejecuta la suite completa 20 veces seguidas
Then no aparece ningún fallo intermitente en los tests de conexión rechazada
