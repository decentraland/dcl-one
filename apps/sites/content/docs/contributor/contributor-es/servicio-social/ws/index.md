# WebSocket

El Social Service es un servidor que gestiona las amistades dentro de Decentraland. Utilizando la plataforma Matrix, sirve como la fuente autorizada para manejar las actualizaciones de Friendships, como solicitudes, aceptaciones, rechazos, cancelaciones y eliminaciones.

### Casos de uso

El Social Service aborda los siguientes casos de uso relacionados con las amistades:

* Solicitar una amistad
* Aceptar una solicitud de amistad
* Rechazar una solicitud de amistad
* Cancelar una solicitud de amistad
* Eliminar un amigo
* Listar todos mis amigos
* Listar amigos en común entre yo y otro usuario

### Contexto

Antes de la introducción del Social Service, los datos de amistad se almacenaban y recuperaban directamente desde Matrix. Esto generaba varios problemas con los flujos de trabajo de amistad y con información poco fiable. Además, implementar nuevas funciones era un desafío. Como Matrix se encarga principalmente de las funciones de chat, el Social Service actúa como un proxy y aplica la lógica de negocio necesaria antes de interactuar con Matrix.

### Arquitectura

#### Historial de versiones

1. [Primera versión](https://adr.decentraland.org/adr/ADR-113): Inicialmente, el Social Service era una API REST que solo ofrecía endpoints GET para listar todos los amigos y las conexiones mutuas. No contaba con funciones relacionadas con actualizaciones ni con el envío de solicitudes.
2. [Segunda versión](https://adr.decentraland.org/adr/ADR-189): Esta versión introdujo actualizaciones en el flujo de amistad. Para abordar estos cambios, se implementó un servidor RPC de WebSocket. Este servidor maneja mensajes esenciales, manteniendo conexiones abiertas con los clientes para proporcionar actualizaciones oportunas. Las conexiones de WebSocket también facilitan funciones como [el seguimiento de presencia](https://github.com/decentraland/adr/blob/420e63926d20166ae832e3de62dd9e3f2370cd49/content/ADR-246-poc-presence.md) dentro del Social Service.

#### URLs

**Dominio .zone**

Servidores del entorno de desarrollo para pruebas y cuando se usan testnets.

* [Iniciar sesión con Matrix](https://social.decentraland.zone)
* [API REST](https://social.decentraland.zone)
* [API de WebSocket](https://rpc-social.decentraland.zone)

**Dominio .org**

Servidores del entorno de producción y cuando se usa la red principal.

* [Iniciar sesión con Matrix](https://social.decentraland.org)
* [API REST](https://social.decentraland.org)
* [API de WebSocket](https://rpc-social.decentraland.org)

### Usa el cliente de JavaScript

Para usar el cliente de JavaScript, consulta <https://github.com/decentraland/social-rpc-client-js#using-the-client>

### Implementaciones de clientes de referencia

* [Cliente de Matrix](https://github.com/decentraland/matrix-client)
* [Cliente de JS](https://github.com/decentraland/social-rpc-client-js)
* [Cliente de referencia de Rust](https://github.com/decentraland/social-client)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/servicio-social/ws.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
