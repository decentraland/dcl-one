# 웹소켓

Social Service는 Decentraland 내의 친구 관계를 관리하는 서버입니다. Matrix 플랫폼을 활용하여 요청, 수락, 거절, 취소, 삭제와 같은 Friendship 업데이트를 처리하는 권위 있는 소스 역할을 합니다.

### 사용 사례

Social Service는 친구 관계와 관련된 다음 사용 사례를 처리합니다:

* 친구 요청
* 친구 요청 수락
* 친구 요청 거절
* 친구 요청 취소
* 친구 삭제
* 내 모든 친구 목록 보기
* 나와 다른 사용자 사이의 공통 친구 목록 보기

### 배경

Social Service가 도입되기 전에는 친구 관계 데이터가 Matrix에서 직접 저장되고 조회되었습니다. 이로 인해 친구 관계 워크플로에서 여러 문제가 발생했고, 정보도 신뢰할 수 없었습니다. 또한 새로운 기능을 구현하는 것도 어려웠습니다. Matrix는 주로 채팅 기능을 처리하므로, Social Service는 Matrix와 상호작용하기 전에 필요한 비즈니스 로직을 적용하는 프록시 역할을 합니다.

### 아키텍처

#### 버전 기록

1. [첫 번째 버전](https://adr.decentraland.org/adr/ADR-113): 처음에는 Social Service가 모든 친구와 상호 연결을 나열하는 GET 엔드포인트만 제공하는 REST API였습니다. 업데이트나 요청 전송과 관련된 기능은 없었습니다.
2. [두 번째 버전](https://adr.decentraland.org/adr/ADR-189): 이 버전에서는 친구 관계 흐름에 대한 업데이트가 도입되었습니다. 이러한 변경 사항에 대응하기 위해 WebSocket RPC 서버가 구현되었습니다. 이 서버는 핵심 메시지를 처리하고, 클라이언트와의 연결을 열린 상태로 유지하여 적시에 업데이트를 제공합니다. WebSocket 연결은 또한 다음과 같은 기능을 가능하게 합니다 [접속 상태 추적](https://github.com/decentraland/adr/blob/420e63926d20166ae832e3de62dd9e3f2370cd49/content/ADR-246-poc-presence.md) Social Service 내에서.

#### URL

**.zone 도메인**

테스트 및 테스트넷 사용 시의 개발 환경 서버.

* [Matrix로 로그인](https://social.decentraland.zone)
* [REST API](https://social.decentraland.zone)
* [WebSocket API](https://rpc-social.decentraland.zone)

**.org 도메인**

메인넷 사용 시의 프로덕션 환경 서버.

* [Matrix로 로그인](https://social.decentraland.org)
* [REST API](https://social.decentraland.org)
* [WebSocket API](https://rpc-social.decentraland.org)

### JavaScript 클라이언트를 사용하려면

JavaScript 클라이언트를 사용하려면 다음을 참조하세요 <https://github.com/decentraland/social-rpc-client-js#using-the-client>

### 참조 클라이언트 구현

* [Matrix 클라이언트](https://github.com/decentraland/matrix-client)
* [JS 클라이언트](https://github.com/decentraland/social-rpc-client-js)
* [Rust 참조 클라이언트](https://github.com/decentraland/social-client)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/social-service/ws.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
