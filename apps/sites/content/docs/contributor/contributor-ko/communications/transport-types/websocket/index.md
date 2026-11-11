# 웹소켓

웹소켓 전송은 Decentraland comms 프로토콜이 지원하는 섬 메시징 전송 방식 중 하나입니다. 클라이언트는 자신의 realm의 [Archipelago](/docs/contributor/contributor-ko/communications/archipelago/) 서비스로부터 받았을 때, 또는 고정 전송으로 표시된 경우에 사용해야 합니다.

웹소켓 전송의 맥락에서, island는 다음과 같이 불립니다: *룸*. 어느 이름이든 같은 것을 의미합니다: 주변에 있는 플레이어들이 업데이트와 채팅을 주고받는 그룹입니다.

연결 문자열은 다음으로 시작합니다 `ws-room:` 접두사에 이어 `wss://` 특정 룸의 URI가 뒤따릅니다. 형식은 다음과 같습니다:

```
ws-room:wss://comms.example.com/rooms/<room-id>
```

웹소켓 URI에 명시적인 프로토콜이 없는 경우, `wss://` 가정됩니다.

### 연결하기

이 전송은 HTTPS 위에서 일반 웹소켓 프로토콜을 사용합니다. 클라이언트는 자신이 사용할 수 있는 어떤 표준 구현으로도 연결을 열 수 있습니다.

### 웹소켓 패킷

웹소켓 전송의 모든 메시지는 다음을 사용해 직렬화됩니다 [`WsPacket`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L78) 구조입니다. 이는 프로토콜 수준의 연결 및 인증 메시지 집합과, 클라이언트 메시지를 위한 컨테이너 타입을 구현합니다.


> [!NOTE]
> 이 [`WsPacket`](#websocket-packets) 구조는 메시지 \[`패킷`]\[Packet]과 혼동하면 안 됩니다. 이는 웹소켓 전송에 특화된 추가 래핑 계층입니다. 실제 comms 메시지는 다음 안에 포함됩니다 [`WsPeerUpdate`](#wspeerupdate) 타입입니다.


| 필드    | 유형    | 값                                                                                                                                                                                                                                                 |
| ----- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `메시지` | `열거형` | <p>다음 중 하나 <code>WsIdentification</code>, <code>WsChallengeRequired</code>, <code>WsSignedChallenge</code>,<br><code>WsWelcome</code>, <code>WsPeerJoin</code>, <code>WsPeerLeave</code>, <code>WsKicked</code> 또는 <code>WsPeerUpdate</code>.</p> |

### 인증

다른 이들에게 메시지를 중계하기 시작하려면, 클라이언트는 챌린지 문자열에 서명하여 인증해야 합니다. 이는 전송 URI를 다음으로부터 얻었을 때도 필요합니다 [Archipelago](/docs/contributor/contributor-ko/communications/archipelago/) 이전 인증 절차 이후에.

클라이언트가 island에 참여할 때 처음 보내는 메시지는 [`WsIdentification`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64), 여기에는 공개 주소가 포함됩니다. 이에 대한 응답으로 [`WsChallengeRequired`](#wschallengerequired), 그리고 클라이언트는 다음을 보냅니다 [`WsSignedChallenge`](#wssignedchallenge) 응답으로.

```
  .----------.                .--------.
  |  서버  |                | 클라이언트 |
  '----+-----'                '---+----'
       ⋮                          |
       ⋮                  연결 |
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

흐름이 성공적으로 완료되면, 클라이언트는 다음을 받게 됩니다 [`WsWelcome`](#wswelcome) 그리고 피어들에게 메시지를 보내기 시작할 수 있습니다.

***

### `WsIdentification` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64) <a href="#wsidentification" id="wsidentification"></a>

| 필드        | 유형       | 값                 |
| --------- | -------- | ----------------- |
| `address` | `string` | 클라이언트의 공개 이더리움 주소 |

***

### `WsChallengeRequired` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L48) <a href="#wschallengerequired" id="wschallengerequired"></a>

| 필드                  | 유형       | 값                                             |
| ------------------- | -------- | --------------------------------------------- |
| `challenge_to_sign` | `string` | 신원 증명으로 서명해야 하는 서버 제공 문자열                     |
| `already_connected` | `string` | 이전 연결이 종료되었을 수 있음을 나타내는, 클라이언트에 대한 서버의 힌트입니다. |

***

### `WsSignedChallenge` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L56) <a href="#wssignedchallenge" id="wssignedchallenge"></a>

| 필드                | 유형       | 값                                                                              |
| ----------------- | -------- | ------------------------------------------------------------------------------ |
| `auth_chain_json` | `string` | 직렬화된 [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 으로 끝납니다. |

***

### `WsWelcome` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L11) <a href="#wswelcome" id="wswelcome"></a>

| 필드                | 유형                    | 값                           |
| ----------------- | --------------------- | --------------------------- |
| `별칭`              | `uint32`              | 클라이언트 세션을 위한 서버 생성 ID       |
| `peer_identities` | `map<uint32, string>` | 현재 모든 피어의 주소이며, 별칭으로 인덱싱됩니다 |

### 연결성

클라이언트가 피어와 자신의 상태를 추적할 수 있도록 세 가지 메시지가 정의되어 있습니다.

[`WsPeerJoin`](#wspeerjoin) 는 다른 클라이언트가 룸에 연결해 인증에 성공했을 때 수신되며, [`WsPeerLeave`](#wspeerleave) 는 피어가 연결을 끊을 때 수신됩니다.

또한 다음이 있습니다 [`WsKicked`](#wskicked) 메시지가 있는데, 이는 클라이언트의 연결이 곧 종료될 것임과 그 이유를 알려줍니다. 현재 관행상 서버가 클라이언트를 룸에서 내쫓는 주된 이유는 서버 정책상 금지되어 있음에도 동시에 다른 룸에 연결했기 때문입니다.

***

### `WsPeerJoin` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L19) <a href="#wspeerjoin" id="wspeerjoin"></a>

| 필드        | 유형       | 값                                           |
| --------- | -------- | ------------------------------------------- |
| `별칭`      | `uint32` | 다음에 전송되는 서버 생성 ID [`WsWelcome`](#wswelcome) |
| `address` | `string` | 새로운 피어의 이더리움 주소                             |

***

### `WsPeerLeave` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L27) <a href="#wspeerleave" id="wspeerleave"></a>

| 필드   | 유형       | 값                   |
| ---- | -------- | ------------------- |
| `별칭` | `uint32` | 연결이 끊긴 피어의 서버 생성 ID |

***

### `WsKicked` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L74) <a href="#wskicked" id="wskicked"></a>

| 필드       | 유형       | 값                      |
| -------- | -------- | ---------------------- |
| `reason` | `string` | 연결이 종료되는 이유에 대한 서버의 설명 |

### 클라이언트 메시지

comms를 보내려면 [메시지](/docs/contributor/contributor-ko/communications/messages/), 클라이언트는 이를 다음 안에 감쌉니다 [`WsPeerUpdate`](#wspeerupdate) 구조입니다. 이는 전송 제어 메시지 유형과 피어 간에 전송되는 실제 메시지를 구분합니다.

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

### `WsPeerUpdate` [**↗ 소스**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L34) <a href="#wspeerupdate" id="wspeerupdate"></a>

| 필드           | 유형       | 값                                                                       |
| ------------ | -------- | ----------------------------------------------------------------------- |
| `from_alias` | `uint32` | 발신자의 서버 생성 ID                                                           |
| `body`       | `바이트`    | 직렬화된 [메시지](/docs/contributor/contributor-ko/communications/messages/) 래핑되는 |
| `비신뢰성`       | `bool`   | 이 메시지를 전달할 때 발신자가 속도와 신뢰성 중 무엇을 우선했는지                                   |

클라이언트는 다음을 설정해야 합니다 `from_alias` 필드를 다음으로 `0` 전송할 때 설정해야 하며, 서버는 이를 피어에게 전달하기 전에 올바른 식별자로 채웁니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transport-types/websocket.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
