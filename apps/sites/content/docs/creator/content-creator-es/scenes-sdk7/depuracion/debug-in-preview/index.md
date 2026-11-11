> Cómo puedes depurar tu escena mientras la ejecutas localmente en la vista previa

# Depurar en la vista previa

Ejecutar una vista previa proporciona información de depuración útil y herramientas para ayudarte a entender cómo se renderiza la escena.

Si la escena no se puede compilar, solo verás el suelo vacío, sin nada renderizado sobre él. Si ocurre esto, hay varios lugares donde puedes buscar mensajes de error para ayudarte a entender qué salió mal:

1. Revisa tu editor de código para asegurarte de que no haya marcado errores de sintaxis o de lógica.
2. Revisa la consola en la ventana de la vista previa para ver si hay mensajes de error. Abre la consola haciendo clic en el ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) icono en la esquina superior derecha.
3. Si ejecutaste la escena usando Creator Hub, revisa la ventana con la salida de la consola de la escena. Si ejecutaste la escena usando la CLI, revisa la salida de la línea de comandos donde ejecutaste `npm run start`


> [!NOTE]
> **💡 Consejo**: Antes de publicar tu escena en el entorno de producción, puedes publicarla en un [Decentraland World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para probarla en un contexto mucho más cercano a producción.


## Usa la consola

Abre la consola haciendo clic en el ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) icono en la esquina superior derecha.

También puedes alternarla presionando la tecla de acento grave de tu teclado: **`**. Esta tecla está a la izquierda de la tecla 1 en la mayoría de los teclados en inglés. O, de lo contrario, presiona Shift + **\`** para abrir una vista más amplia de la consola, por si necesitas ver más texto.

Envía mensajes a la consola (usando `console.log()`). Luego puedes ver estos mensajes a medida que se generan abriendo la consola de la escena.

La consola también muestra mensajes de error; estos se marcan en rojo.

## El panel de depuración

Abre la consola de depuración haciendo clic en el ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) icono en la esquina superior derecha.

Esto abre un panel en la esquina inferior derecha con información sobre el motor de renderizado, y se actualiza en tiempo real a medida que cambian las cosas.

![](/docs/_remote/eace962a130964afb6774cff.webp)

### Escena actual

El **ESCENA ACTUAL** la pestaña incluye información sobre la escena en la que te encuentras actualmente. Esto incluye:

* La cantidad de parcelas en la escena
* La altura máxima a la que se te permite construir (330 metros para cada escena)

También puedes consultar la **Mostrar límites de la escena** opción para ver paredes rojas alrededor de la escena, que marcan los límites de la escena.

### Memoria

El **MEMORIA** la sección muestra la memoria utilizada por Decentraland. Intenta mantener siempre este valor por debajo de los **umbrales del presupuesto de memoria**. Si este valor aumenta demasiado, corre el riesgo de provocar que la aplicación se bloquee para los jugadores que visitan tu escena. Ten en cuenta que, en producción, la escena podría cargarse junto con otras escenas y jugadores cercanos, lo que puede contribuir a aumentar el consumo de memoria por encima de lo que ves en la vista previa.

### Rendimiento

El **RENDIMIENTO** la pestaña incluye varios campos relacionados con *FPS* (Frames Per Second). Este valor tenderá a ser más bajo a medida que la escena gane complejidad; asegúrate de mantener siempre este valor por encima de 25 FPS para garantizar una buena experiencia para tus jugadores. El FPS variará según la máquina en la que lo ejecutes.

El valor más importante aquí es el **FPS promedio**, que redondea el FPS de los últimos 1000 frames.

![](/docs/_remote/5ba9631495e357c71fac976e.webp)


> [!WARNING]
> **📔 Nota**: El FPS del renderer no es lo mismo que las actualizaciones por segundo del game loop de la escena. La lógica de la escena está limitada a 30 FPS; el FPS del renderer puede llegar a 60 o más.


Los parones se refieren a pausas momentáneas en la tasa de fotogramas, que pueden tener poco impacto en el número total de FPS pero ser muy notables para el jugador. Suelen ocurrir cuando se cargan assets pesados o algo similar.

### Solicitudes web

El **SOLICITUDES WEB** la sección te permite abrir la pestaña Network de Chrome Devtools, que muestra la cantidad de solicitudes web realizadas por la escena y su contenido. Esto es útil para saber si la escena está haciendo demasiadas solicitudes a servidores, lo que puede afectar el rendimiento de la escena.

Si tu escena interactúa con un servidor de terceros, a menudo resulta útil ver qué datos entran y salen. Haz clic en **Abrir Chrome Devtools** para abrir una nueva ventana de Chrome con la pestaña Network abierta.


> [!WARNING]
> **📔 Nota**: Para usar esta función, primero debes instalar el [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/), ya que las dependencias necesarias vienen incluidas con esa instalación.
> 
> No necesitas tener Creator Hub abierto ni ejecutar la vista previa de tu escena a través de Creator Hub para usar esta función; también puedes usarla si estás usando la CLI para ejecutar la vista previa de tu escena.


Como alternativa, hay varias herramientas gratuitas que puedes ejecutar en tu máquina y que informan de todos los datos que entran y salen de la aplicación de Decentraland. Por ejemplo [Charles](https://www.charlesproxy.com/) o [Wireshark](https://www.wireshark.org/).

## Recarga rápida

Cada vez que haces cambios en la escena, la vista previa se recarga y se actualiza automáticamente, así que no hay necesidad de abrir y cerrar la ventana de vista previa.

Si necesitas recargar la escena sobre la que estás parado, haz clic en el **Recargar escena** botón en la esquina superior izquierda de la ventana de vista previa.

![](/docs/_remote/a2ec4266edb8ce7bb450b24d.webp)

También puedes recargar la escena escribiendo lo siguiente en el chat y pulsando Enter:

`/reload`

## Informar de un error

Si encuentras un problema que no esté en tu escena, sino en el Decentraland SDK en general, consulta [Informar de un error](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/).

## Versiones de las dependencias

Asegúrate de usar siempre las versiones más recientes de todas las dependencias de tu escena, ya que cualquier problema que estés experimentando podría estar ya corregido en versiones más nuevas. Creator Hub te avisará cuando haya actualizaciones para instalar en tu escena de Decentraland.

![](/docs/_remote/a002cdf10ca3a37130f2959d.webp)

También puedes actualizar manualmente las dependencias mediante la línea de comandos. Ejecutar una escena de Decentraland localmente depende de dos librerías principales: `@dcl/sdk` y `@dcl/js-runtime`, que se instala en cada carpeta del proyecto. Asegúrate de que ambas estén actualizadas. Puedes ejecutar los siguientes comandos para actualizar manualmente ambas librerías a la última versión estable:

```
npm i @dcl/js-runtime@latest
npm i @dcl/sdk@latest
```

Si tu escena usa smart items, también puede estar usando la `@dcl/asset-packs` librería, que puedes actualizar con:

```
npm i @dcl/asset-packs@latest
```

Si estás usando alguna de las [librerías utils](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) asegúrate de que también estén actualizadas, ya que las versiones antiguas de estas librerías pueden no ser compatibles con versiones más recientes de `@dcl/sdk`.

## Condiciones de iluminación

La hora del día en el mundo tiene un gran impacto en cómo se ven los modelos 3D. El color de la fuente de luz cambia sutilmente, con un tono azulado por la noche y un tono rojizo durante el amanecer y el atardecer. La dirección de la luz también se desplaza por el cielo, proyectando sombras en distintas direcciones.

Comprueba que tu escena se vea bien a todas horas cambiando el reloj del juego a distintos valores. Haz clic en el **hora del skybox** icono del panel izquierdo de la pantalla para establecer la hora que prefieras. Si este control deslizante está atenuado, asegúrate de que la **Auto** opción esté desactivada.

<figure><img src="/docs/_remote/be169c2886b8a8e49dfe5d5d.webp" alt="Dynamic skybox" width="300"><figcaption><p>Instrucciones dentro del mundo</p></figcaption></figure>

Los materiales de tu modelo 3D pueden no verse igual que en la herramienta de modelado con la que lo creaste. Esto es esperable, ya que todos los motores de renderizado 3D tienen diferencias sutiles en cómo manejan la luz y los materiales.

También puedes configurar tu escena para que la hora del día quede fija y la iluminación no cambie. Consulta [control del skybox](/docs/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control/) para más detalles.

## Pruebas multijugador

Si inicias una vista previa de la escena y la abres en dos (o más) ventanas de Explorer diferentes, cada ventana abierta se interpretará como un jugador distinto, y un servidor de comunicaciones simulado mantendrá sincronizados a estos jugadores.

Interactúa con la escena en una ventana y luego cambia a la otra para ver que los efectos de esa interacción también son visibles allí.

Usando Creator Hub, activa **Vista previa multinstancia** en el **cliente de escritorio** submenú de **Opciones de juego**, y luego haz clic en el **Jugar** botón por segunda vez para abrir una segunda ventana de Explorer de Decentraland. Debes conectarte en ambas ventanas con direcciones diferentes. Las mismas sesiones permanecerán abiertas mientras la escena se recarga. Puede que necesites cerrar sesión y volver a iniciar sesión en la segunda ventana para seleccionar una cuenta diferente.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Ambos jugadores se verán entre sí y podrán interactuar el uno con el otro. Si la escena tiene alguna lógica multijugador, funcionará como se espera y se sincronizará entre los dos jugadores.

Como alternativa, puedes abrir una segunda ventana de Explorer de Decentraland escribiendo lo siguiente en una URL del navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Usar la red de pruebas de Ethereum

Mientras pruebas tu escena, para evitar transferir MANA real u otras monedas, puedes usar la red de pruebas Sepolia de Ethereum y transferir MANA falso de la testnet en su lugar.

Para usar la red de pruebas debes configurar tu extensión de Chrome Metamask para que use la *red de pruebas Sepolia* en lugar de *red principal*. También debes tener MANA en la blockchain de Sepolia, que puedes obtener gratis en Decentraland.


> [!NOTE]
> **💡 Consejo**: Para ejecutar la transacción de transferir MANA de Sepolia a tu Wallet, deberás pagar una tarifa de gas en Ether de Sepolia.


Cualquier transacción que aceptes mientras ves la escena en este modo solo ocurrirá en la red de pruebas y no afectará el saldo de MANA en tu Wallet real.

Para previsualizar tu escena usando la red de pruebas, pega la siguiente URL en una pestaña del navegador. Esto abrirá la escena en el cliente de escritorio de Decentraland:

> decentraland://realm=<http://127.0.0.1:8000\&local-scene=true\&debug=true\&dclenv=zone\&position=0,0>


> [!NOTE]
> **💡 Consejo**: Cambia el parámetro position a las coordenadas de tu escena, para cargar directamente en tu escena.. Cualquier transacción que aceptes mientras ves la escena en este modo solo ocurrirá en la red de pruebas y no afectará el saldo de MANA en tu Wallet real.


Si necesitas probar transacciones en la Polygon Testnet y necesitas tener MANA en esa testnet, tendrás que intercambiar MANA a esa red después de obtenerlo en Sepolia. Para transferir MANA de Sepolia a la Polygon Testnet, visita la página de tu cuenta de Decentraland en Sepolia y haz clic en ‘swap’ en el lado de Ethereum MANA.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
