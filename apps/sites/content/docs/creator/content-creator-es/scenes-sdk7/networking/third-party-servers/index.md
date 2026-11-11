> Crea escenas multijugador de Decentraland con un servidor de terceros.

# Servidores de terceros

La forma recomendada de abordar las interacciones multijugador en tu escena es usar [Servidor Multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/). Este documento cubre otras opciones que también son compatibles.

* **REST API + DB**: Bueno para datos que cambian con poca frecuencia (clasificaciones, libros de visitas). Los jugadores consultan la API periódicamente para obtener actualizaciones; el estado persiste entre sesiones. Ver [Conexiones de red](/docs/creator/content-creator-es/scenes-sdk7/networking/network-connections/) sobre cómo hacer `fetch` solicitudes desde una escena.
* **servidor WebSocket**: Permite la comunicación bidireccional en tiempo real. Ver [Conexiones de red](/docs/creator/content-creator-es/scenes-sdk7/networking/network-connections/) para el uso de WebSocket. Bibliotecas como [Colyseus](https://colyseus.io/) funcionan bien con el SDK de Decentraland.


> [!WARNING]
> **📔 Nota**: Los servidores de terceros no se integran con `syncEntity`, `validateBeforeChange`, o `Almacenamiento` — tendrás que implementar tu propia gestión del estado y lógica de sincronización.


También es posible optar por un enfoque híbrido en el que los cambios se notifican entre los jugadores mediante [serverless-multiplayer](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/), pero el estado final también se almacena mediante una REST API para futuros visitantes.

Aquí tienes algunos ejemplos de escenas que usan un servidor de terceros:

* [Clasificación](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)
* [Libro de visitas](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)
* [Validar autenticidad](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity)

## Probar localmente

Para previsualizar una escena que usa un servidor de terceros, debes ejecutar tanto la escena como el servidor del que depende. El servidor puede ejecutarse localmente en la misma máquina que la Preview, como una forma más sencilla de probarlo. Cuando se ejecuta localmente, el servidor puede usar conexiones ws inseguras `http` o `ws` conexiones, para una configuración más sencilla.

Para iniciar el servidor, ve a la `/server` carpeta y ejecuta `npm run start`.

Abre el Preview en dos ventanas separadas; cada ventana se trata como un jugador independiente. Conecta cada ventana con una dirección diferente. Ambos clientes se conectarán a la misma instancia del servidor local.

Usando Creator Hub, activa **Previsualización de múltiples instancias** en el **Cliente de escritorio** submenú de **Opciones de reproducción**, luego haz clic en **Reproducir** botón por segunda vez para abrir una segunda ventana del Explorer de Decentraland. Debes conectarte en ambas ventanas con direcciones diferentes. Las mismas sesiones permanecerán abiertas mientras la escena se recarga.

Como alternativa, puedes abrir una segunda ventana del Explorer de Decentraland escribiendo lo siguiente en una URL del navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Realms separadas

Los jugadores en Decentraland existen en muchos *realms* separados. Los jugadores en diferentes realms no pueden verse, interactuar ni chatear entre sí, incluso si están de pie sobre las mismas parcelas. Dividir a los jugadores de esta manera permite que Decentraland maneje una cantidad ilimitada de jugadores sin encontrar ninguna limitación. También empareja a los jugadores que están en regiones cercanas para garantizar que los tiempos de ping entre los jugadores que interactúan sean aceptables.

Si tu escena envía datos a un servidor de terceros para sincronizar cambios entre jugadores en tiempo real, entonces es importante que los cambios solo se sincronicen entre jugadores que están en el mismo realm. Debes tratar todos los cambios que pertenecen a un realm como separados de los de un realm diferente. De lo contrario, los jugadores verán cómo las cosas cambian de forma inquietante, sin que nadie haga el cambio.

Consulta cómo obtener el realm de cada jugador en [obtener datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/)

## Persistencia multijugador

A diferencia de las escenas locales que se montan de nuevo cada vez que un jugador entra en ellas, las escenas que usan servidores de terceros tienen una vida útil que se extiende mucho más allá de cuando el jugador entra y sale de la escena.

Por lo tanto, debes diseñar la experiencia teniendo en cuenta que el jugador no siempre encontrará la escena en el mismo estado inicial. Cualquier cambio realizado en la escena permanecerá para que otros jugadores lo encuentren; debes asegurarte de que estos no interfieran de manera no deseada con las experiencias de los jugadores futuros.

### Restablecer el estado

Al cargar la escena, asegúrate de que se construya en función de la información compartida almacenada en el servidor, y no en un estado predeterminado.

En algunos casos, tiene sentido incluir algún tipo de botón de reinicio en la escena. Al presionar el botón de reinicio, la escena se restablecería de manera ordenada.

A veces, esto solo implica volver a establecer las variables del estado de la escena a sus valores predeterminados. Pero restablecer la escena también podría implicar anular la suscripción de listeners y detener loops en el lado del servidor. Si quedan loops vacíos cada vez que se restablece la escena, estos se irían acumulando y tendrían un efecto negativo en el rendimiento de la escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/networking/third-party-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
