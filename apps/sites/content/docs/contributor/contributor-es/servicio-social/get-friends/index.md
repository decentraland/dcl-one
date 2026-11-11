# Obtener amigos

Detalles sobre cómo usar la Social Service API para recuperar amistades, solicitudes y amigos en común en la plataforma Decentraland. La API permite a los desarrolladores interactuar con Social Service para recuperar información relacionada con amistades.

#### API REST

Para recuperar amistades, Social Service expone un [API REST](https://social-service-api-specs.pages.dev/):

* `GET /friendships/me` - Recuperar las amistades del usuario actual. Requiere el token de acceso en el encabezado Authorization.
* `GET /friendships/{other-address}/mutuals` - Recuperar amigos en común con otro usuario. Requiere el token de acceso en el encabezado Authorization.

#### Servidor RPC de WebSocket

La API WebSocket de Social Service proporciona dos tipos de mensajes para recuperar amistades y amigos en común. Primero debes establecer una conexión WebSocket y luego usar esos métodos para transmitir información relacionada con el usuario.

* `GetFriends` - Recuperar los amigos de un usuario. Devuelve un stream de respuestas de usuario.
* `GetMutualFriends` - Recuperar amigos en común con otro usuario. Devuelve un stream de respuestas de usuario.

Consulta el [archivo friendships.proto](https://github.com/decentraland/protocol/blob/main/proto/decentraland/social/friendships/friendships.proto#L7) para obtener más detalles sobre estos tipos de mensaje.

### Ejemplos de código en JavaScript

Para interactuar con la API WebSocket de Social Service usando JavaScript, puedes usar el cliente RPC proporcionado, como se explicó antes. Pero si necesitas implementarlo por tu cuenta, a continuación hay ejemplos de código que muestran cómo recuperar amigos y amigos en común:

#### Obtener todos los amigos

```javascript
import { createRpcClient, createWebSocketsTransport } from '@dcl/rpc/dist/client';
import { FriendshipsServiceDefinition } from './protobuff-types/decentraland/social/friendships/friendships.gen';

const socialClientRpcUrl = 'wss://social.decentraland.org'; // Reemplaza por la URL real
const webSocketsTransport = createWebSocketsTransport(socialClientRpcUrl);
const service = loadService(FriendshipsServiceDefinition, webSocketsTransport);

const response = service.getFriends(Payload.create({ synapseToken }));
for await (const friends of response) {
  processErrors(friends);
  const userList = friends.users?.users ?? [];
  // Procesar la lista de amigos
}
```

#### Obtener amigos en común

```javascript
const response = service.getMutualFriends(
  MutualFriendsPayload.create({
    user: { address }, 
    authToken: { synapseToken }
  })
);
for await (const mutualFriend of response) {
  processErrors(mutualFriend);
  const mutualFriendList = mutualFriend.users?.users ?? [];
  // Procesar la lista de amigos en común
}
```

Para ver más ejemplos de código y más detalles, visita el [repositorio de GitHub social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/servicio-social/get-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
