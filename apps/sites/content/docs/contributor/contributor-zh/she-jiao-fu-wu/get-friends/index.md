# 获取好友

关于在 Decentraland 平台中使用 Social Service API 检索好友关系、请求和共同好友的详细信息。该 API 允许开发者与 Social Service 交互，以获取与好友关系相关的信息。

#### REST API

要检索好友关系，Social Service 提供了一个 [REST API](https://social-service-api-specs.pages.dev/):

* `GET /friendships/me` - 检索当前用户的好友关系。需要在 Authorization 请求头中提供访问令牌。
* `GET /friendships/{other-address}/mutuals` - 检索与另一位用户的共同好友。需要在 Authorization 请求头中提供访问令牌。

#### WebSocket RPC 服务器

Social Service WebSocket API 提供两种用于检索好友关系和共同好友的消息类型。你需要先建立 WebSocket 连接，然后使用这些方法来流式获取与用户相关的信息。

* `GetFriends` - 检索某个用户的好友。返回一个用户响应流。
* `GetMutualFriends` - 检索与另一位用户的共同好友。返回一个用户响应流。

请参阅 [friendships.proto 文件](https://github.com/decentraland/protocol/blob/main/proto/decentraland/social/friendships/friendships.proto#L7) 以了解有关这些消息类型的更多详情。

### JavaScript 代码示例

要使用 JavaScript 与 Social Service WebSocket API 交互，你可以使用前面说明的 RPC 客户端。不过，如果你需要自行实现，下面提供了演示如何检索好友和共同好友的代码示例：

#### 获取所有好友

```javascript
import { createRpcClient, createWebSocketsTransport } from '@dcl/rpc/dist/client';
import { FriendshipsServiceDefinition } from './protobuff-types/decentraland/social/friendships/friendships.gen';

const socialClientRpcUrl = 'wss://social.decentraland.org'; // 替换为实际 URL
const webSocketsTransport = createWebSocketsTransport(socialClientRpcUrl);
const service = loadService(FriendshipsServiceDefinition, webSocketsTransport);

const response = service.getFriends(Payload.create({ synapseToken }));
for await (const friends of response) {
  processErrors(friends);
  const userList = friends.users?.users ?? [];
  // 处理好友列表
}
```

#### 获取共同好友

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
  // 处理共同好友列表
}
```

如需更多代码示例和详细信息，请访问 [social-rpc-client-js GitHub 仓库](https://github.com/decentraland/social-rpc-client-js)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/she-jiao-fu-wu/get-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
