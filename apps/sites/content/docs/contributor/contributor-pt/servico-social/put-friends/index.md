# Atualizar amigos

Interaja com a API do Social Service para realizar ações relacionadas a eventos e usuários. Essas ações incluem criar novas solicitações de amizade, aceitar ou rejeitar solicitações recebidas e excluir amizades. O Social Service permite que os usuários gerenciem seus relacionamentos e interações dentro da plataforma.

### Amizades e Ações

As amizades dentro do Social Service são relacionamentos bidirecionais entre usuários. Podem ser realizadas ações para atualizar essas amizades, como enviar solicitações de amizade, aceitar ou rejeitar solicitações, cancelar solicitações e remover amizades. Cada uma dessas ações tem implicações específicas para o relacionamento entre os usuários.

### Implementando Atualizações de Amizade

Para realizar essas ações relacionadas a amizades, a `UpdateFriendshipEvent` mensagem precisa ser utilizada. Essa mensagem permite a execução de várias ações relacionadas a amizades, como solicitar, aceitar, rejeitar, cancelar e remover.

A estrutura exata do payload e os parâmetros necessários para cada ação estão especificados na `UpdateFriendshipEvent` mensagem.

**Exemplo de Uso**

Aqui está um exemplo de como você pode usar a `UpdateFriendshipEvent` para enviar uma solicitação de amizade:

```proto
message UpdateFriendshipEvent {
  string userAddress = 1;
  string targetUserAddress = 2;
  FriendshipAction action = 3;
}
```

Neste exemplo, `userAddress` representa o endereço do remetente, `targetUserAddress` representa o endereço do destinatário, e `action` indica o tipo de ação a ser executada (solicitar, aceitar, rejeitar, cancelar ou remover).

#### Enviando Solicitações de Amizade

Qualquer usuário pode enviar uma solicitação de amizade para outro usuário. No entanto, para estabelecer uma amizade, o usuário destinatário deve aceitar a solicitação.

```javascript
requestFriendship: async (address: string, message?: string) => {
  const response = await service.updateFriendshipEvent({
    event: { request: { user: { address }, message } },
    authToken: { synapseToken }
  })
  processErrors(response)
  return response.event
}
```

#### Aceitando ou Rejeitando Solicitações

Quando um usuário recebe uma solicitação de amizade, ele tem a opção de aceitá-la ou rejeitá-la. Aceitar uma solicitação estabelece uma amizade mútua entre os usuários. Rejeitar uma solicitação impede o estabelecimento de uma amizade.

```javascript
acceptFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      event: { accept: { user: { address } } },
      authToken: { synapseToken }
    })
  )
  processErrors(response)
  return response.event
}
```

```javascript
rejectFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      authToken: { synapseToken },
      event: { reject: { user: { address } } }
    })
  )
  processErrors(response)
  return response.event
}
```

#### Cancelando Solicitações de Amizade

Se um usuário decidir retirar uma solicitação de amizade enviada antes que ela seja aceita, ele pode cancelar a solicitação. Isso impede que o destinatário veja ou responda à solicitação.

```javascript
cancelFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      event: { cancel: { user: { address } } },
      authToken: { synapseToken }
    })
  )
  processErrors(response)
  return response.event
}
```

#### Removendo Amizades

Qualquer um dos usuários em uma amizade mútua tem a opção de remover a amizade. Quando uma amizade é removida, ela é excluída dos registros de ambos os usuários, encerrando o relacionamento mútuo.

```javascript
removeFriend: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      authToken: { synapseToken },
      event: { delete: { user: { address } } }
    })
  )
  processErrors(response)
  return response.event
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/servico-social/put-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
