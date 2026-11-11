# Realm MAIN

Decentraland tiene tantos realms como Catalyst con servicios de comunicación, pero hay un realm distintivo conocido como MAIN que existe fuera de la red de Catalysts. El realm MAIN fue diseñado específicamente para agilizar el proceso de conexión de los usuarios, facilitando que interactúen y se encuentren. Está construido con un conjunto de servicios capaces de sustituir el servicio de comunicación de un nodo Catalyst. Este realm sirve como destino predeterminado para los usuarios al iniciar sesión en Decentraland.

<img src="/docs/_remote/d01dbf731e2a22a53eeb9e6a.webp" alt="" width="800">

El [proveedor de realm](https://github.com/decentraland/realm-provider) los servicios exponen un [/about](https://realm-provider.decentraland.org/main/about) endpoint con una descripción del realm según el [ADR-110: Descripción del realm](https://adr.decentraland.org/adr/ADR-110). Esto proporciona la información para establecer una conexión con la red de gestión de contenido, conocida como uno de los servicios del nodo Catalyst, y también suministra los detalles de conexión para los servicios de comunicación del realm MAIN.

Se accede al servicio de comunicaciones MAIN a través de la [Conector WebSocket](https://github.com/decentraland/archipelago-workers). Este componente ligero permite la interacción con el resto de los componentes del backend y puede escalarse fácilmente con instancias adicionales para acomodar una carga de usuarios en expansión, gestionando eficientemente un mayor número de conexiones activas. A través de la conexión WebSocket, el cliente obtiene los datos necesarios para establecer una conexión con LiveKit Cloud, junto con el `islandId` asignado por el Archipelago Service. El Archipelago Service tiene responsabilidades idénticas tanto en el realm MAIN como en el servicio de comunicaciones de los nodos Catalyst. Gestiona eficazmente el procesamiento de las posiciones de los usuarios y su posterior asignación a islas distintas, cada una de las cuales corresponde a una sala de LiveKit, organizando así eficientemente los grupos de usuarios según sus ubicaciones espaciales dentro del mundo virtual. Para más información sobre el Archipelago Service, puedes consultar la [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) página.

[NATS](https://nats.io/) funciona como un sistema de difusión que facilita la comunicación fluida entre varios servicios. Puede soportar múltiples instancias de WebSocket Connector que se comuniquen con el mismo Archipelago Service. Además, permite que el servicio de Stats recopile estadísticas de usuarios de Archipelago, como posiciones y agrupaciones de islas, haciendo que estos datos estén disponibles para su utilización externa.

El servicio de estadísticas de Archipelago comparte la misma implementación de API que el servicio Archipelago en el nodo Catalyst. Puedes explorar la API en este enlace: [Especificaciones de la API de Catalyst](https://decentraland.github.io/catalyst-api-specs/#tag/Archipelago). El servicio es accesible a través de la siguiente URL `https://archipelago-stats.decentraland.org/` y los endpoints disponibles están bajo la `/comms/` como se describe en las especificaciones de la API, por ejemplo: `https://archipelago-stats.decentraland.org/comms/peers`.

El [LiveKit](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/livekit/) La plataforma Cloud gestiona la compleja tarea de intercambiar la información de los usuarios mediante WebRTC y es la principal plataforma Cloud compartida por la mayoría de los nodos Catalyst para el mismo propósito.

El concepto fundamental derivado del realm MAIN es que diferentes usuarios tienen la flexibilidad de conectarse a cualquier nodo Catalyst con fines de gestión de contenido (descargar escenas, wearables, emotes o enviar actualizaciones de perfil), pero a pesar de la selección de un nodo específico de gestión de contenido, la conexión al servicio de comunicación **puede** ser la misma. Como resultado, incluso si los usuarios están utilizando diferentes nodos Catalyst como servidores de contenido pero se encuentran en coordenadas cercanas de la escena, tendrán la capacidad de interactuar entre sí.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/main-realm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
