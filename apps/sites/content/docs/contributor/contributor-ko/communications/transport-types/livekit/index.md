# LiveKit

이 [라이브킷](https://livekit.io/) 이 transport는 지원되는 transport 중 가장 다재다능하고 효율적이며, 구현도 가장 쉽습니다. 클라이언트는 해당 realm의 [Archipelago](/docs/contributor/contributor-ko/communications/archipelago/) 서비스에서 적절한 연결 문자열을 받았거나, 고정 transport로 표시된 경우 이를 사용해야 합니다.


> [!NOTE]
> 오픈 소스 [Comms Station](https://decentraland.github.io/comms-station/).


커스텀 [웹소켓](/docs/contributor/contributor-ko/communications/transport-types/websocket/) transport와 달리, 제3자(오픈 소스) 라이브러리를 사용하므로 모든 transport 수준의 로직과 직렬화를 추상화하여 개발 과정을 크게 단순화합니다.

LiveKit transport URI는 `livekit:` 접두사로 시작하며, 그 뒤에 `wss://` 서버 URL이 이어집니다. 또한 사전 승인된 `access_token`을 포함하며, 이는 [Archipelago](/docs/contributor/contributor-ko/communications/archipelago/) 클라이언트가 인증할 때 생성됩니다.

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

### 연결 및 인증

LiveKit 클라이언트 라이브러리는 모든 연결 및 인증 로직을 깔끔하게 캡슐화합니다.

LiveKit transport를 사용할 때는 추가 인증 단계가 없습니다.  `access_token` Archipelago에서 제공한 는 이미 검증되고 승인되었습니다.

### 메시지 교환

LiveKit 라이브러리에 의존하는 클라이언트는 손쉽게 브로드캐스트하고, 직접 메시지를 보내고, 라이브 미디어 피드를 게시할 수 있습니다.

페이로드는 일반 통신 [`패킷이며`](/docs/contributor/contributor-ko/communications/messages/) 추가적인 래핑이 없습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transport-types/livekit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
