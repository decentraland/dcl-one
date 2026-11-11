# Transportes

Un *transporte* es una interfaz del lado del cliente que puede conectarse a un URI de comms e intercambiar mensajes en tiempo real con pares o servicios.


> [!NOTE]
> Puedes ver el protocolo de comms en acción y experimentarlo usando el software de código abierto [Comms Station](https://decentraland.github.io/comms-station/).


Los clientes son libres de implementar transportes como consideren conveniente (siempre que sigan el protocolo esperado por otros clientes y servicios), pero el [diseño recomendado](#recommended) que se describe a continuación se ha probado en la práctica y ha demostrado integrarse bien con distintos proyectos.\`

### Transportes estándar <a href="#types" id="types"></a>

Hay 5 tipos de transporte definidos por el protocolo:

* [Websocket](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/websocket/) (`ws-room`) usa un flujo estándar de websocket.
* [LiveKit](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/livekit/) (`livekit`) usa el framework LiveKit de código abierto, basado en WebRTC.
* [SignedLogin](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/signed-login/) (`signed-login`) realiza una solicitud HTTPs firmada para obtener un transporte asignado dinámicamente desde un servidor.
* [Offline](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/offline/) (`offline`) es un transporte ficticio e indica que no hay comms en el entorno actual.
* [Simulator](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/simulator/) (`simulator`) es un transporte ficticio con un comportamiento completamente personalizado, principalmente para que los desarrolladores depuren sus implementaciones.

Los mensajes enviados y recibidos a través de una interfaz de transporte son siempre los mismos (véase [mensajes](/docs/contributor/contributor-es/comunicaciones/messages/)）。 Algunos transportes pueden envolverlos en estructuras de control para su transmisión, pero estas deben desempaquetarse antes de cruzar la `Transport` interfaz.

### Diseño recomendado <a href="#recommended" id="recommended"></a>

Este es un `Transport` interfaz escrita en TypeScript:

```ts
interface Transport {
  // Inicializa el Transport con un URI:
  constructor(private uriWithConnectionParams: string)

  // Abre una conexión usando el URI dado en el constructor:
  connect(): Promise<void>

  // Cierra la conexión:
  disconnect(): Promise<void>

  // Envía una carga útil arbitraria al servicio y a todos los pares:
  send(packet: Packet): Promise<void>

  // Suscríbete a los mensajes entrantes tanto del servicio como de todos los pares:
  on(event: 'receive', callback: (packet: Packet) => void): void
}
```

Las implementaciones reales suelen extender esto, añadiendo eventos de conexión/desconexión y de unión/salida, envío sin difusión, tipado más estricto o adaptaciones específicas del lenguaje.

#### URI de transporte

Los valores de `uriWithConnectionParams` siempre tienen la forma:

```
<type>:<type connection parameters>
```

El `<type>` corresponde a uno de los enumerados arriba, y el formato de `<type connection parameters>` depende del transporte específico. Puede ser una URL, un token opaco o cualquier dato arbitrario.

El [LiveKit](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/livekit/) el transporte, por ejemplo, usa una `wss` URL con un `access_token` parámetro para establecer una conexión autenticada:

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

Como comparación, el [Websocket](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/websocket/) transporte también usa una `wss` URL, pero sin un token pre-generado. Requiere un flujo de autenticación una vez conectado.

#### Creación de transportes

Dado que cada `Transport` clase admitirá un tipo particular de URI, es buena idea tener un método de fábrica que devuelva un `Transport` o falle de inmediato. Por ejemplo:

```ts
function createTransport(uriWithConnectionParams: string) {
  const type = getPrefix(urlWithConnectionParams)

  switch (type) {
    case 'ws-room': return new WsRoomTransport(uriWithConnectionParams)
    case 'livekit': return new LiveKitTransport(uriWithConnectionParams)
    // ... otras implementaciones compatibles...
  }

  throw new Error(`Tipo de transporte no compatible: ${type}`)
}
```

Los clientes no están obligados a implementar todos los transportes estándar. Si solo pretenden usar un subconjunto de los tipos definidos sin intentar manejar todos los URI, son libres de rechazar los tipos que no admitan.

Se recomienda que los clientes con más funciones, como un World Explorer, implementen al menos el [websocket](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/websocket/) y [LiveKit](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/livekit/) transportes, que actualmente están en uso en todos los realms principales.

### Más información

Dirígete a una [tipo de transporte específico](#types) sección, lee sobre los diferentes [tipos de mensaje](/docs/contributor/contributor-es/comunicaciones/messages/) o consulta \[\[Comms Demo Station]].


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/transports.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
