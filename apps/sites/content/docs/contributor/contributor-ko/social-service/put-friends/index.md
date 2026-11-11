# 친구 추가하기

소셜 서비스 API와 상호작용하여 이벤트와 사용자와 관련된 작업을 수행합니다. 이러한 작업에는 새 친구 요청 생성, 들어오는 요청 수락 또는 거절, 그리고 친구 관계 삭제가 포함됩니다. 소셜 서비스는 사용자가 플랫폼 내에서 관계와 상호작용을 관리할 수 있도록 합니다.

### 친구 관계와 작업

소셜 서비스 내의 친구 관계는 사용자 간의 양방향 관계입니다. 친구 요청 보내기, 요청 수락 또는 거절, 요청 취소, 친구 관계 삭제와 같은 작업을 통해 이러한 친구 관계를 업데이트할 수 있습니다. 이러한 각 작업은 사용자 간 관계에 특정한 영향을 미칩니다.

### 친구 관계 업데이트 구현

이러한 친구 관계 관련 작업을 수행하려면, `UpdateFriendshipEvent` 메시지를 사용해야 합니다. 이 메시지를 통해 요청, 수락, 거절, 취소, 삭제와 같이 친구 관계와 관련된 다양한 작업을 실행할 수 있습니다.

각 작업에 필요한 정확한 페이로드 구조와 필수 매개변수는 `UpdateFriendshipEvent` 메시지에 명시되어 있습니다.

**사용 예시**

다음은 `UpdateFriendshipEvent` 를 사용해 친구 요청을 보내는 방법의 예입니다:

```proto
message UpdateFriendshipEvent {
  string userAddress = 1;
  string targetUserAddress = 2;
  FriendshipAction action = 3;
}
```

이 예시에서, `userAddress` 는 발신자의 주소를 나타내고, `targetUserAddress` 는 수신자의 주소를 나타내며, `action` 은 수행할 작업의 유형(요청, 수락, 거절, 취소 또는 삭제)을 나타냅니다.

#### 친구 요청 보내기

모든 사용자는 다른 사용자에게 친구 요청을 보낼 수 있습니다. 하지만 친구 관계를 성립하려면 수신한 사용자가 요청을 수락해야 합니다.

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

#### 요청 수락 또는 거절

사용자가 친구 요청을 받으면, 이를 수락하거나 거절할 수 있습니다. 요청을 수락하면 사용자 간 상호 친구 관계가 성립합니다. 요청을 거절하면 친구 관계 성립이 거부됩니다.

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

#### 친구 요청 취소하기

사용자가 수락되기 전에 보낸 친구 요청을 철회하기로 결정하면, 요청을 취소할 수 있습니다. 이렇게 하면 수신자가 요청을 보거나 응답하는 것을 방지할 수 있습니다.

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

#### 친구 관계 삭제하기

상호 친구 관계에 있는 어느 사용자든 친구 관계를 삭제할 수 있습니다. 친구 관계가 삭제되면 양쪽 사용자의 기록에서 삭제되어 상호 관계가 종료됩니다.

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
GET https://docs.decentraland.org/contributor/contributor-ko/social-service/put-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
