# 아키펠라고

Archipelago는 인근 플레이어들을 섬으로 그룹화하고, 이동에 따라 재배치하며, 메시지를 중계할 실제 백엔드에 연결하는 데 필요한 정보를 제공하는 realm 서비스입니다.


> [!NOTE]
> 오픈 소스인 Archipelago 프로토콜의 동작을 확인하고 직접 실험해 볼 수 있습니다. [Comms Station](https://decentraland.github.io/comms-station/).


이 서비스를 사용하려면, 클라이언트는 자신의 realm의 Archipelago 웹소켓 엔드포인트에 연결하고 인증하여 세션을 시작해야 합니다. 그런 다음 위치 업데이트 전송과 섬 할당 수신을 시작할 수 있습니다(다음을 참조 [클라이언트 생명 주기](/docs/contributor/contributor-ko/communications/overview/#lifecycle)).

Archipelago 서비스와 주고받는 모든 메시지는 Decentraland에서 정의한 대로 프로토콜 버퍼를 사용해 인코딩됩니다. [프로토콜 저장소](https://github.com/decentraland/protocol).

### 연결하기 <a href="#connecting" id="connecting"></a>

시작하려면, 클라이언트는 보안 웹소켓 연결(`wss:`)을 `/archipelago/ws` realm 엔드포인트에 열어야 합니다.

연결이 완료되면, 클라이언트는 realm 정책에서 정의한 시간 창(기본값은 60초) 내에 [인증](#authenticating) 흐름의 각 메시지를 전송해야 합니다.


> [!WARNING]
> 더 큰 realm은 Archipelago 서비스를 실행하지만, 더 작은 realm은 모든 플레이어에게 고정된 백엔드 연결 문자열을 제공하기로 선택할 수 있습니다. URI가 있으면 다음에서 찾을 수 있습니다. `comms.fixedAdapter` realm의 `/about`.


이 경우 동적인 섬 할당은 사용할 수 없으며, 해당 목적에 realm의 RPC 인터페이스를 사용해서는 안 됩니다.

### 인증하기 <a href="#authenticating" id="authenticating"></a>

Archipelago에 연결한 후, 클라이언트는 먼저 서비스에 챌린지를 요청하고 서명하여 자신의 신원을 검증해야 합니다.

클라이언트가 보내는 첫 번째 메시지는 [`ChallengeRequestMessage`](#challengerequestmessage) 이며, 자신의 이더리움 주소(즉, 공개 키)를 함께 보냅니다. 그러면 [`ChallengeResponseMessage`](#challengeresponsemessage) 를 받아 서명할 무작위 문자열을 받게 되고, [`SignedChallengeMessage`](#signedchallengemessage).

를 보내 응답해야 합니다. [`SignedChallengeMessage`](#signedchallengemessage) The [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 을 포함하며, 제공된 주소로 시작해 챌린지 서명으로 끝납니다.

서비스가 서명을 성공적으로 검증하면, 클라이언트는 인증되며 [`WelcomeMessage`](#welcomemessage).

```mermaid
sequenceDiagram
    participant Client
    participant Archipelago
    
    Client,Archipelago 위에 주석: 연결
    Client->>Archipelago: ChallengeRequestMessage
    Archipelago->>Client: ChallengeResponseMessage
    Client->>Archipelago: SignedChallengeMessage
    Archipelago->>Client: WelcomeMessage
```

### 하트비트 전송 <a href="#heartbeat" id="heartbeat"></a>

세션 동안 클라이언트는 주기적으로 [`Heartbeat`](#heartbeat-message) 메시지를 보내 Archipelago가 섬 할당을 발급하는 데 필요한 정보를 최신 상태로 유지해야 합니다.


> [!NOTE]
> 커뮤니케이션 클라이언트에 권장되는 하트비트 빈도는 대략 초당 한 번의 업데이트입니다.


클라이언트가 [`Heartbeat`](#heartbeat-message) 메시지 전송을 중단하면, Archipelago는(현재 정책에 따라) 연결을 종료할 수 있습니다.

### 섬 할당 받기 <a href="#assignment" id="assignment"></a>

첫 번째 하트비트 직후, Archipelago는 클라이언트에게 첫 번째 [`IslandChangedMessage`](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L17).

를 전송합니다. `conn_str`는 전송 계층을 초기화하고 섬에 연결하는 데 사용할 수 있습니다. 값은 일반적으로 다음과 같습니다:

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

첫 번째 `:` 앞의 레이블은 전송 유형이며, 나머지는 이를 위한 특수 URI입니다. 여기에는 사전 승인된 토큰이나 다른 매개변수가 포함될 수 있습니다.

세션 동안 Archipelago는 여러 이유로 언제든지 새 할당을 보낼 수 있습니다.

1. 위치 변경: 클라이언트가 섬 내 다른 사람들로부터 멀어졌다고 보고했습니다.
2. 섬 요청: 클라이언트가 특정 섬에 할당되길 요청했습니다.
3. Archipelago 정책: 서비스가 인구 균형을 더 잘 맞추기 위해 섬을 생성하거나 분할하기로 결정했습니다.

클라이언트는 이러한 할당을 수신하도록 대기하고, 지시된 대로 전송 연결을 닫았다가 열며, 필요할 경우 사용 중인 전송 유형을 변경해야 합니다.

### 클라이언트 메시지

#### `ChallengeRequestMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L54) <a href="#challengerequestmessage" id="challengerequestmessage"></a>

인증 흐름을 시작하기 위해 세션의 첫 번째 메시지로 클라이언트가 보냅니다.

| 필드        | 유형       | 값           |
| --------- | -------- | ----------- |
| `address` | `string` | 사용자의 주소입니다. |

를 보내 응답해야 합니다. `address` field는 다음에 제시될 대상의 첫 번째 개인 키에서 파생되어야 합니다. [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 에서 제시될 것입니다.

***

#### `SignedChallengeMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L58) <a href="#signedchallengemessage" id="signedchallengemessage"></a>

클라이언트가 [`ChallengeResponseMessage`](#challengeresponsemessage)를 받은 후 보내며, 인증 흐름을 완료하기 위해 사용됩니다.

| 필드                | 유형       | 값                                                                                     |
| ----------------- | -------- | ------------------------------------------------------------------------------------- |
| `auth_chain_json` | `string` | JSON으로 직렬화된 [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 으로 끝납니다. |

의 첫 번째 키는 [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 원래 보낸 주소와 일치해야 합니다. [`ChallengeRequestMessage`](#challengerequestmessage).

***

#### `Heartbeat` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L62) <a href="#heartbeat-message" id="heartbeat-message"></a>

클라이언트가 일정한 간격으로(보통 초당 한 번) 보내며, 자신의 위치를 Archipelago에 업데이트하고/또는 섬 할당을 요청합니다.

| 필드             | 유형        | 값                       |
| -------------- | --------- | ----------------------- |
| `position`     | `위치`      | 월드 맵에서 클라이언트의 3D 위치     |
| `desired_room` | `string?` | 클라이언트가 할당받고 싶어 하는 섬의 ID |

첫 번째 `Heartbeat` 메시지는 곧바로 다음으로 이어집니다. [`IslandChangedMessage`](#islandchangedmessage) Archipelago에서 온 것입니다. 그러나 이후의 업데이트는 섬 할당과 무관합니다. 클라이언트는 `Heartbeat` 에 대한 응답이 오리라 기대해서는 안 됩니다.

다음이 포함되면 `desired_room` 매개변수가 포함되면 서비스는 요청을 수용하려고 시도하지만, 해당 섬으로의 재할당이 보장되지는 않습니다. 이는 Archipelago의 정책(예: 섬 인구 제한)에 따라 달라집니다.

### 서버 메시지

#### `ChallengeResponseMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L8) <a href="#challengeresponsemessage" id="challengeresponsemessage"></a>

Archipelago가 [`ChallengeRequestMessage`](#challengerequestmessage)

| 필드                  | 유형       | 값                                                                     |
| ------------------- | -------- | --------------------------------------------------------------------- |
| `challenge_to_sign` | `string` | 서명하고 [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) |
| `already_connected` | `bool`   | 이 사용자의 키에 대한 기존 연결이                                                   |

***

#### `WelcomeMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L13) <a href="#welcomemessage" id="welcomemessage"></a>

인증 성공 후 Archipelago가 보냅니다.

| 필드        | 유형       | 값                        |
| --------- | -------- | ------------------------ |
| `peer_id` | `string` | 인증된 클라이언트의 고유 식별자(보통 주소) |

***

#### `IslandChangedMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L17) <a href="#islandchangedmessage" id="islandchangedmessage"></a>

클라이언트가 섬에 (재)할당될 때 Archipelago가 보냅니다.

설명.

| 필드               | 유형                      | 값                                                                                |
| ---------------- | ----------------------- | -------------------------------------------------------------------------------- |
| `island_id`      | `string`                | 새 섬의 ID                                                                          |
| `from_island_id` | `string?`               | 이 재할당인 경우 이전 섬의 ID                                                               |
| `conn_str`       | `string`                | 섬의 연결 문자열 [transport](/docs/contributor/contributor-ko/communications/transports/). |
| `peers`          | `map<string, Position>` | 설명.                                                                              |

다음을 받는 클라이언트는 `IslandChangedMessage` 섬 백엔드에 대한 연결을 종료하고, `conn_str`.

를 보내 응답해야 합니다. `peers` 필드에는 섬 안의 모든 피어의 현재 신원과 위치가 포함되어 있어 클라이언트가 초기 집합을 채울 수 있습니다. 이 시점 이후에는 섬 [transport](/docs/contributor/contributor-ko/communications/transports/) 를 통해 수신한 메시지에 의존해 위치 업데이트를 받아야 합니다.

***

#### `KickedMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#LL37C1-L38C1) <a href="#kickedmessage" id="kickedmessage"></a>

연결을 닫기 전에 Archipelago가 보냅니다.

| 필드       | 유형             | 값                      |
| -------- | -------------- | ---------------------- |
| `reason` | `KickedReason` | 연결을 닫는 Archipelago의 사유 |

다음에 대한 표준 값은 `reason` 필드는 다음과 같습니다:

* `KR_NEW_SESSION` : 동일한 키로 인증된 다른 연결입니다.

***

#### `JoinIslandMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L29) <a href="#joinislandmessage" id="joinislandmessage"></a>

피어가 클라이언트의 섬에 할당될 때 Archipelago가 보냅니다.

| 필드          | 유형       | 값                 |
| ----------- | -------- | ----------------- |
| `island_id` | `string` | 섬의 식별자            |
| `peer_id`   | `string` | 피어의 고유 식별자(보통 주소) |

를 보내 응답해야 합니다. `island_id` 필드는 클라이언트의 현재 할당과 일치합니다.

***

#### `LeftIslandMessage` [↗ 소스](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L24) <a href="#leftislandmessage" id="leftislandmessage"></a>

피어가 클라이언트의 섬에서 제거될 때 Archipelago가 보냅니다.

| 필드          | 유형       | 값                 |
| ----------- | -------- | ----------------- |
| `island_id` | `string` | 섬의 식별자            |
| `peer_id`   | `string` | 피어의 고유 식별자(보통 주소) |

를 보내 응답해야 합니다. `island_id` 필드는 클라이언트의 현재 할당과 일치합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/archipelago.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
