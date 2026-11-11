# WebSocket

El transporte websocket es uno de los transportes de mensajería de isla compatibles con el protocolo de comms de Decentraland. Los clientes deben usarlo cuando reciban una cadena de conexión adecuada de la service de su realm [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) o si se indica como un transporte fijo.

En el contexto del transporte websocket, una isla se denomina *sala*. Cualquiera de los dos nombres indica lo mismo: un grupo de jugadores cercanos que intercambian actualizaciones y chat.

Las cadenas de conexión comienzan con el `ws-room:` prefijo, seguido de una `wss://` URI para la sala específica. Se ven así:

```
ws-room:wss://comms.example.com/rooms/<room-id>
```

A falta de un protocolo explícito para la URI websocket, `wss://` se asume HTTPS.

### Conectando

El transporte usa el protocolo websocket estándar sobre HTTPS. Los clientes pueden abrir una conexión usando cualquier implementación estándar a su disposición.

### Paquetes de Websocket

Todos los mensajes del transporte websocket se serializan usando la [`WsPacket`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L78) estructura. Implementan un conjunto de mensajes de conectividad y autenticación a nivel de protocolo, además de un tipo contenedor para los mensajes del cliente.


> [!NOTE]
> El [`WsPacket`](#websocket-packets) la estructura no debe confundirse con el mensaje \[`Packet`]\[Packet]. Es una capa de encapsulado adicional específica del transporte websocket. Los mensajes reales de comms están contenidos en el [`WsPeerUpdate`](#wspeerupdate) tipo.


| Campo     | Tipo   | Valor                                                                                                                                                                                                                                           |
| --------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mensaje` | `enum` | <p>Uno de <code>WsIdentification</code>, <code>WsChallengeRequired</code>, <code>WsSignedChallenge</code>,<br><code>WsWelcome</code>, <code>WsPeerJoin</code>, <code>WsPeerLeave</code>, <code>WsKicked</code> o <code>WsPeerUpdate</code>.</p> |

### Autenticación

Antes de poder empezar a reenviar mensajes a otros, los clientes deben autenticarse firmando una cadena de desafío. Esto se requiere incluso cuando la URI del transporte se obtuvo de [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) después de una ronda previa de autenticación.

El primer mensaje que un cliente envía al unirse a una isla es [`WsIdentification`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64), que contiene una dirección pública. Se responderá con [`WsChallengeRequired`](#wschallengerequired), y los clientes envían un [`WsSignedChallenge`](#wssignedchallenge) en respuesta.

```
  .----------.                .--------.
  |  Server  |                | Client |
  '----+-----'                '---+----'
       ⋮                          |
       ⋮                  Conectar |
       o - - - - - - - - - - - - -|
       |                          |
       |                          |
       |         WsIdentification |
       |<-------------------------+
       +------------------------->|
       | WsChallengeRequired      |
       |                          |
       |                          |
       |        WsSignedChallenge |
       |<-------------------------+
       +------------------------->|
       | WsWelcome                |
       |                          |

```

Si el flujo se completa correctamente, el cliente recibirá un [`WsWelcome`](#wswelcome) y podrá empezar a enviar mensajes a sus pares.

***

### `WsIdentification` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64) <a href="#wsidentification" id="wsidentification"></a>

| Campo     | Tipo     | Valor                                        |
| --------- | -------- | -------------------------------------------- |
| `address` | `string` | La dirección pública de Ethereum del cliente |

***

### `WsChallengeRequired` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L48) <a href="#wschallengerequired" id="wschallengerequired"></a>

| Campo               | Tipo     | Valor                                                                                     |
| ------------------- | -------- | ----------------------------------------------------------------------------------------- |
| `challenge_to_sign` | `string` | La cadena proporcionada por el servidor que debe firmarse como prueba de identidad        |
| `already_connected` | `string` | Una indicación del servidor a los clientes de que las conexiones previas pueden cerrarse. |

***

### `WsSignedChallenge` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L56) <a href="#wssignedchallenge" id="wssignedchallenge"></a>

| Campo             | Tipo     | Valor                                                                                                                                                            |
| ----------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `auth_chain_json` | `string` | Una versión serializada [cadena de autenticación](/docs/contributor/contributor-es/autenticacion/authchain/) serializada en JSON que termina con la firma del reto. |

***

### `WsWelcome` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L11) <a href="#wswelcome" id="wswelcome"></a>

| Campo             | Tipo                  | Valor                                                            |
| ----------------- | --------------------- | ---------------------------------------------------------------- |
| `alias`           | `uint32`              | Un ID generado por el servidor para la sesión del cliente        |
| `peer_identities` | `map<uint32, string>` | Las direcciones de todos los pares actuales, indexadas por alias |

### Conectividad

Se definen tres mensajes para ayudar a los clientes a hacer seguimiento de sus pares y de su propio estado.

[`WsPeerJoin`](#wspeerjoin) se recibe cuando otro cliente se conecta a la sala y se autentica correctamente, mientras que [`WsPeerLeave`](#wspeerleave) se recibe cuando los pares se desconectan.

También está el [`WsKicked`](#wskicked) mensaje, que informa a los clientes de que su conexión está a punto de cerrarse y de la razón para ello. En la práctica actual, la razón principal por la que un servidor puede expulsar a un cliente de una sala es porque se ha conectado simultáneamente a otra sala, cuando eso está prohibido por la política del servidor.

***

### `WsPeerJoin` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L19) <a href="#wspeerjoin" id="wspeerjoin"></a>

| Campo     | Tipo     | Valor                                                               |
| --------- | -------- | ------------------------------------------------------------------- |
| `alias`   | `uint32` | El ID generado por el servidor enviado en [`WsWelcome`](#wswelcome) |
| `address` | `string` | La dirección de Ethereum del nuevo par                              |

***

### `WsPeerLeave` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L27) <a href="#wspeerleave" id="wspeerleave"></a>

| Campo   | Tipo     | Valor                                               |
| ------- | -------- | --------------------------------------------------- |
| `alias` | `uint32` | El ID generado por el servidor del par desconectado |

***

### `WsKicked` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L74) <a href="#wskicked" id="wskicked"></a>

| Campo    | Tipo     | Valor                                                         |
| -------- | -------- | ------------------------------------------------------------- |
| `reason` | `string` | La explicación del servidor de por qué se cerrará la conexión |

### Mensajes del cliente

Para enviar comms [mensajes](/docs/contributor/contributor-es/comunicaciones/messages/), los clientes los envuelven en la [`WsPeerUpdate`](#wspeerupdate) estructura. Esto diferencia los tipos de mensajes de control del transporte de los mensajes reales enviados entre pares.

```
.-------------------.
| WsPacket          |
|  .--------------. |
|  | WsPeerUpdate | |
|  | .----------. | |
|  | |  Packet  | | |
|  | '----------' | |
|  '--------------' |
'-------------------'

```

***

### `WsPeerUpdate` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L34) <a href="#wspeerupdate" id="wspeerupdate"></a>

| Campo        | Tipo     | Valor                                                                                                    |
| ------------ | -------- | -------------------------------------------------------------------------------------------------------- |
| `from_alias` | `uint32` | El ID generado por el servidor del remitente                                                             |
| `body`       | `bytes`  | El serializado [mensaje](/docs/contributor/contributor-es/comunicaciones/messages/) que se está envolviendo |
| `no fiable`  | `bool`   | Si el remitente priorizó la velocidad o la fiabilidad para entregar este mensaje                         |

Los clientes deben establecer el `from_alias` campo en `0` al enviar, y el servidor lo completará con el identificador correcto antes de entregarlo a los pares.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/tipos-de-transporte/websocket.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
