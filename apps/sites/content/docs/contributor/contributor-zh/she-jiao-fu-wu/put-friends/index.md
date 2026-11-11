# 设置好友

与社交服务 API 交互，以执行与事件和用户相关的操作。这些操作包括创建新的好友请求、接受或拒绝收到的请求，以及删除好友关系。社交服务允许用户在平台内管理他们的关系和互动。

### 好友关系与操作

社交服务中的好友关系是用户之间的双向关系。可以执行操作来更新这些好友关系，例如发送好友请求、接受或拒绝请求、取消请求以及移除好友关系。每种操作都会对用户之间的关系产生特定影响。

### 实现好友关系更新

要执行这些与好友关系相关的操作，必须使用 `UpdateFriendshipEvent` 消息。该消息允许执行与好友关系相关的各种操作，例如请求、接受、拒绝、取消和移除。

每个操作所需的确切负载结构和参数在 `UpdateFriendshipEvent` 消息中指定。

**使用示例**

下面是一个如何使用 `UpdateFriendshipEvent` 来发送好友请求的示例：

```proto
message UpdateFriendshipEvent {
  string userAddress = 1;
  string targetUserAddress = 2;
  FriendshipAction action = 3;
}
```

在这个示例中， `userAddress` 表示发送方地址， `targetUserAddress` 表示接收方地址，而 `action` 表示要执行的操作类型（请求、接受、拒绝、取消或移除）。

#### 发送好友请求

任何用户都可以向另一个用户发送好友请求。不过，要建立好友关系，接收方用户必须接受该请求。

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

#### 接受或拒绝请求

当用户收到好友请求时，他们可以选择接受或拒绝。接受请求会在用户之间建立互惠好友关系。拒绝请求则不会建立好友关系。

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

#### 取消好友请求

如果用户决定在好友请求被接受之前撤回已发送的请求，他们可以取消该请求。这可以阻止接收方看到或响应该请求。

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

#### 移除好友关系

双向好友关系中的任一用户都可以选择移除该好友关系。好友关系被移除后，它会从双方用户的记录中删除，从而结束互惠关系。

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
GET https://docs.decentraland.org/contributor/contributor-zh/she-jiao-fu-wu/put-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
