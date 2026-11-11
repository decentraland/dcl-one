# 전송

하나의 *전송* 는 클라이언트 측 인터페이스로, comms URI에 연결하여 피어 또는 서비스와 실시간 메시지를 주고받을 수 있습니다.


> [!NOTE]
> 오픈 소스 [Comms Station](https://decentraland.github.io/comms-station/).


클라이언트는 다른 클라이언트와 서비스가 기대하는 프로토콜만 따르는 한, 전송을 원하는 방식으로 자유롭게 구현할 수 있지만, [권장 설계](#recommended) 아래에 설명된 방식은 실제로 테스트되었으며 서로 다른 프로젝트와도 잘 통합되는 것으로 입증되었습니다.\`

### 표준 전송 <a href="#types" id="types"></a>

프로토콜에 정의된 전송 유형은 5가지입니다:

* [웹소켓](/docs/contributor/contributor-ko/communications/transport-types/websocket/) (`ws-room`)는 표준 웹소켓 스트림을 사용합니다.
* [라이브킷](/docs/contributor/contributor-ko/communications/transport-types/livekit/) (`livekit`)는 WebRTC를 기반으로 한 오픈 소스 LiveKit 프레임워크를 사용합니다.
* [서명 로그인](/docs/contributor/contributor-ko/communications/transport-types/signed-login/) (`signed-login`)는 서명된 HTTPS 요청을 보내 서버에서 동적으로 할당된 전송을 가져옵니다.
* [오프라인](/docs/contributor/contributor-ko/communications/transport-types/offline/) (`offline`)는 현재 환경에 통신이 없음을 나타내는 더미 전송입니다.
* [시뮬레이터](/docs/contributor/contributor-ko/communications/transport-types/simulator/) (`simulator`)는 주로 개발자가 구현을 디버그하기 위해 완전히 사용자 정의된 동작을 가진 더미 전송입니다.

전송 인터페이스를 통해 전송되고 수신되는 메시지는 항상 동일합니다(참조: [메시지](/docs/contributor/contributor-ko/communications/messages/)). 일부 전송은 전송을 위해 메시지를 제어 구조로 감쌀 수 있지만, 이는 `Transport` 인터페이스를 넘기기 전에 풀어야 합니다.

### 권장 설계 <a href="#recommended" id="recommended"></a>

이것은 최소한의 `Transport` TypeScript로 작성된 인터페이스입니다:

```ts
interface Transport {
  // URI로 Transport를 초기화합니다:
  constructor(private uriWithConnectionParams: string)

  // 생성자에서 주어진 URI를 사용해 연결을 엽니다:
  connect(): Promise<void>

  // 연결을 닫습니다:
  disconnect(): Promise<void>

  // 서비스와 모든 피어에게 임의의 페이로드를 전송합니다:
  send(packet: Packet): Promise<void>

  // 서비스와 모든 피어로부터 들어오는 메시지를 구독합니다:
  on(event: 'receive', callback: (packet: Packet) => void): void
}
```

실제 구현은 일반적으로 이를 확장하여 연결/연결 해제 및 참여/퇴장 이벤트, 브로드캐스트하지 않는 전송, 더 엄격한 타입 지정 또는 언어별 적응을 추가합니다.

#### 전송 URI

uriWithConnectionParams의 값은 항상 다음 형식입니다: `uriWithConnectionParams` 는 항상 다음 형식입니다:

```
<type>:<type connection parameters>
```

이 `<type>` 는 위에 나열된 것 중 하나에 해당하며, `<type connection parameters>` 의 형식은 특정 전송에 따라 다릅니다. URL일 수도 있고, 불투명한 토큰 또는 임의의 데이터일 수도 있습니다.

이 [라이브킷](/docs/contributor/contributor-ko/communications/transport-types/livekit/) 전송은, 예를 들어, `wss` URL과 `access_token` 매개변수를 사용해 인증된 연결을 설정합니다:

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

비교하자면, [웹소켓](/docs/contributor/contributor-ko/communications/transport-types/websocket/) 전송도 `wss` URL을 사용하지만 사전 생성된 토큰은 없습니다. 연결되면 인증 흐름이 필요합니다.

#### 전송 만들기

각 `Transport` 클래스가 특정 종류의 URI를 지원하므로, 유효한 `Transport` 을 반환하거나 즉시 실패하는 팩토리 메서드를 두는 것이 좋습니다. 예를 들면:

```ts
function createTransport(uriWithConnectionParams: string) {
  const type = getPrefix(urlWithConnectionParams)

  switch (type) {
    case 'ws-room': return new WsRoomTransport(uriWithConnectionParams)
    case 'livekit': return new LiveKitTransport(uriWithConnectionParams)
    // ... 기타 지원되는 구현...
  }

  throw new Error(`지원되지 않는 전송 유형: ${type}`)
}
```

클라이언트가 모든 표준 전송을 구현할 필요는 없습니다. 정의된 유형의 일부만 사용하고 모든 URI를 처리하려는 목적이 없다면, 지원하지 않는 유형을 거부해도 됩니다.

World Explorer와 같은 더 기능이 풍부한 클라이언트는 현재 모든 주요 렐름에서 사용 중인 [웹소켓](/docs/contributor/contributor-ko/communications/transport-types/websocket/) 및 [라이브킷](/docs/contributor/contributor-ko/communications/transport-types/livekit/) 전송은 최소한 구현할 것을 권장합니다.

### 더 알아보기

특정 [전송 유형](#types) 섹션으로 이동해 다양한 [메시지 유형](/docs/contributor/contributor-ko/communications/messages/) 에 대해 읽어보거나 \[\[Comms Demo Station]]을 확인해 보세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transports.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
