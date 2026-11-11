# Obter amigos

Detalhes sobre como usar a API do Social Service para recuperar amizades, pedidos e amigos em comum na plataforma Decentraland. A API permite que os developers interajam com o Social Service para recuperar informações relacionadas com amizades.

#### REST API

Para recuperar amizades, o Social Service expõe um [REST API](https://social-service-api-specs.pages.dev/):

* `GET /friendships/me` - Recupera as amizades do utilizador atual. Requer o token de acesso no cabeçalho Authorization.
* `GET /friendships/{other-address}/mutuals` - Recupera amigos em comum com outro utilizador. Requer o token de acesso no cabeçalho Authorization.

#### Servidor RPC WebSocket

A API WebSocket do Social Service disponibiliza dois tipos de mensagem para recuperar amizades e amigos em comum. Primeiro, é necessário estabelecer uma ligação WebSocket e depois usar esses métodos para transmitir informações relacionadas com o utilizador.

* `Obter amigos` - Recupera os amigos de um utilizador. Devolve um stream de respostas do utilizador.
* `Obter amigos em comum` - Recupera amigos em comum com outro utilizador. Devolve um stream de respostas do utilizador.

Consulte o [ficheiro friendships.proto](https://github.com/decentraland/protocol/blob/main/proto/decentraland/social/friendships/friendships.proto#L7) para mais detalhes sobre estes tipos de mensagem.

### Exemplos de código JavaScript

Para interagir com a API WebSocket do Social Service usando JavaScript, pode usar o cliente RPC fornecido, como explicado anteriormente. Mas, se precisar de o implementar por si, abaixo estão exemplos de código que demonstram como recuperar amigos e amigos em comum:

#### Obter todos os amigos

```javascript
import { createRpcClient, createWebSocketsTransport } from '@dcl/rpc/dist/client';
import { FriendshipsServiceDefinition } from './protobuff-types/decentraland/social/friendships/friendships.gen';

const socialClientRpcUrl = 'wss://social.decentraland.org'; // Substitua pelo URL real
const webSocketsTransport = createWebSocketsTransport(socialClientRpcUrl);
const service = loadService(FriendshipsServiceDefinition, webSocketsTransport);

const response = service.getFriends(Payload.create({ synapseToken }));
for await (const friends of response) {
  processErrors(friends);
  const userList = friends.users?.users ?? [];
  // Processar a lista de amigos
}
```

#### A obter amigos em comum

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
  // Processar a lista de amigos em comum
}
```

Para mais exemplos de código e detalhes, visite o [repositório GitHub social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/servico-social/get-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
