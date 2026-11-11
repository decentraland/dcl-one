# LiveKit

El [LiveKit](https://livekit.io/) El transporte es el más versátil y eficiente de los transportes compatibles, y también el más fácil de implementar. Los clientes deberían usarlo cuando reciban una cadena de conexión apropiada del servicio de su realm [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) servicio, o si está indicado como un transporte fijo.


> [!NOTE]
> Puedes ver el transporte de LiveKit en acción y probar sus funciones usando el código abierto [Comms Station](https://decentraland.github.io/comms-station/).


A diferencia del [websocket](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/websocket/) transporte personalizado, utiliza una biblioteca de terceros (de código abierto), lo que simplifica enormemente el proceso de desarrollo al abstraer toda la lógica a nivel de transporte y la serialización.

Las URI del transporte de LiveKit comienzan con el `livekit:` prefijo, seguido por la `wss://` URL del servidor. También incluyen un `access_token`, generado por [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) cuando el cliente se autentica.

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

### Conectarse y autenticarse

La biblioteca cliente de LiveKit encapsula elegantemente toda la lógica de conexión y autenticación.

No hay un paso adicional de autenticación cuando se usa el transporte de LiveKit. El `access_token` proporcionado por Archipelago ya está verificado y autorizado.

### Intercambiar mensajes

Los clientes que dependen de la biblioteca de LiveKit pueden transmitir fácilmente, enviar mensajes directos y publicar transmisiones de medios en vivo.

Las cargas útiles son simples comms [`Paquetes`](/docs/contributor/contributor-es/comunicaciones/messages/) sin ningún envoltorio adicional.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/tipos-de-transporte/livekit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
