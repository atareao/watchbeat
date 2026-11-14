## ADDED Requirements

### Requirement: ICMP disponible para usuario no-root

La imagen de contenedor SHALL permitir que el proceso de WatchBeat, ejecutado
como usuario no-root, envíe ICMP. Para ello SHALL instalar `iputils` y aplicar
`setcap cap_net_raw+ep` al binario `ping`, y la configuración de compose SHALL
añadir la capacidad `NET_RAW`.

#### Scenario: ping como usuario no-root en el contenedor
Given la imagen construida y el contenedor arrancado con `cap_add: [NET_RAW]`
When el usuario `app` ejecuta `ping -c 1 <host>`
Then el comando termina con código 0
And no aparece "permission denied"
