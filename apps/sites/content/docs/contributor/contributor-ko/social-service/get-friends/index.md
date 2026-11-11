# 친구 가져오기

Decentraland 플랫폼에서 Social Service API를 사용해 친구 관계, 요청, 상호 친구를 가져오는 방법에 대한 विवरण입니다. 이 API를 통해 개발자는 Social Service와 상호작용하여 친구 관계 관련 정보를 가져올 수 있습니다.

#### REST API

친구 관계를 가져오기 위해 Social Service는 다음을 제공합니다. [REST API](https://social-service-api-specs.pages.dev/):

* `GET /friendships/me` - 현재 사용자의 친구 관계를 가져옵니다. Authorization 헤더에 액세스 토큰이 필요합니다.
* `GET /friendships/{other-address}/mutuals` - 다른 사용자와의 상호 친구를 가져옵니다. Authorization 헤더에 액세스 토큰이 필요합니다.

#### WebSocket RPC 서버

Social Service WebSocket API는 친구 관계와 상호 친구를 가져오기 위한 두 가지 메시지 유형을 제공합니다. 먼저 WebSocket 연결을 설정한 다음, 해당 메서드를 사용해 사용자 관련 정보를 스트리밍해야 합니다.

* `GetFriends` - 사용자의 친구를 가져옵니다. 사용자 응답 스트림을 반환합니다.
* `GetMutualFriends` - 다른 사용자와의 상호 친구를 가져옵니다. 사용자 응답 스트림을 반환합니다.

다음 항목을 참조하세요. [friendships.proto 파일](https://github.com/decentraland/protocol/blob/main/proto/decentraland/social/friendships/friendships.proto#L7) 이 메시지 유형에 대한 자세한 내용은.

### JavaScript 코드 예제

JavaScript를 사용해 Social Service WebSocket API와 상호작용하려면, 앞서 설명한 제공된 RPC 클라이언트를 사용할 수 있습니다. 하지만 직접 구현해야 한다면, 아래에 친구와 상호 친구를 가져오는 방법을 보여주는 코드 예제가 있습니다:

#### 모든 친구 가져오기

```javascript
import { createRpcClient, createWebSocketsTransport } from '@dcl/rpc/dist/client';
import { FriendshipsServiceDefinition } from './protobuff-types/decentraland/social/friendships/friendships.gen';

const socialClientRpcUrl = 'wss://social.decentraland.org'; // 실제 URL로 바꾸세요
const webSocketsTransport = createWebSocketsTransport(socialClientRpcUrl);
const service = loadService(FriendshipsServiceDefinition, webSocketsTransport);

const response = service.getFriends(Payload.create({ synapseToken }));
for await (const friends of response) {
  processErrors(friends);
  const userList = friends.users?.users ?? [];
  // 친구 목록을 처리합니다
}
```

#### 상호 친구 가져오기

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
  // 상호 친구 목록을 처리합니다
}
```

더 많은 코드 예제와 자세한 내용은 다음을 방문하세요. [social-rpc-client-js GitHub 저장소](https://github.com/decentraland/social-rpc-client-js)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/social-service/get-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
