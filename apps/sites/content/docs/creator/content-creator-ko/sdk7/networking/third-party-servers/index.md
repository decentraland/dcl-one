> 서드파티 서버를 사용해 멀티플레이어 Decentraland 씬을 구축합니다.

# 서드파티 서버

씬에서 멀티플레이어 상호작용에 권장되는 접근 방식은 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/). 이 문서에서는 지원되는 다른 옵션도 다룹니다.

* **REST API + DB**: 자주 변경되지 않는 데이터(순위표, 방명록)에 적합합니다. 플레이어는 업데이트를 위해 API를 주기적으로 조회하며, 상태는 세션 간에 유지됩니다. 참고 [네트워크 연결](/docs/creator/content-creator-ko/sdk7/networking/network-connections/) 요청을 만드는 방법은 `fetch` 씬에서 요청을 보내는 방법.
* **WebSocket 서버**: 실시간 양방향 통신을 가능하게 합니다. 참고 [네트워크 연결](/docs/creator/content-creator-ko/sdk7/networking/network-connections/) WebSocket 사용법은. 다음과 같은 라이브러리 [Colyseus](https://colyseus.io/) 는 디센트럴랜드 SDK와 잘 호환됩니다.


> [!WARNING]
> **📔 참고**: 서드파티 서버는 `syncEntity`, `validateBeforeChange`, 또는 `저장소` — 자체 상태 관리 및 동기화 로직을 직접 구현해야 합니다.


변경 사항이 플레이어 간에 다음을 통해 알림되는 하이브리드 방식을 선택하는 것도 가능합니다 [serverless-multiplayer](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/), 하지만 최종 상태는 향후 방문자를 위해 REST API를 통해서도 저장됩니다.

서드파티 서버를 사용하는 씬의 몇 가지 예시는 다음과 같습니다:

* [순위표](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)
* [방명록](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)
* [진위 확인](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity)

## 로컬에서 테스트

서드파티 서버를 사용하는 씬을 미리 보려면 씬과 그 씬이 의존하는 서버를 모두 실행해야 합니다. 서버는 더 쉽게 테스트할 수 있도록 미리보기와 같은 머신에서 로컬로 실행할 수 있습니다. 로컬로 실행할 때 서버는 안전하지 않은 `http` 또는 `ws` 연결을 사용할 수 있어 설정이 더 쉽습니다.

서버를 시작하려면 다음으로 이동하세요: `/server` 폴더로 이동한 다음 실행하세요 `npm run start`.

미리보기를 두 개의 별도 창에서 여세요. 각 창은 별도의 플레이어로 취급됩니다. 각 창을 서로 다른 주소로 연결하세요. 두 클라이언트는 동일한 로컬 서버 인스턴스에 연결됩니다.

Creator Hub를 사용하여 다음을 켜세요. **다중 인스턴스 미리 보기** 을 **데스크톱 클라이언트** 의 하위 메뉴에서 **재생 옵션**, 그런 다음 다음을 클릭하세요. **재생** 버튼을 한 번 더 눌러 두 번째 Decentraland explorer 창을 여세요. 두 창 모두 다른 주소로 연결해야 합니다. 장면이 다시 로드되어도 동일한 세션은 계속 열려 있습니다.

대안으로, 브라우저 URL에 다음을 입력해 두 번째 Decentraland Explorer 창을 열 수 있습니다:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 별도의 렐름

디센트럴랜드의 플레이어들은 여러 개의 서로 분리된 *영역에 존재하며*. 서로 다른 렐름의 플레이어들은 같은 파셀 위에 서 있어도 서로를 볼 수도, 상호작용할 수도, 채팅할 수도 없습니다. 이렇게 플레이어를 분리하면 디센트럴랜드는 어떤 제한에도 걸리지 않고 무제한의 플레이어를 처리할 수 있습니다. 또한 가까운 지역에 있는 플레이어들을 짝지어, 상호작용하는 플레이어 간의 핑 시간이 허용 가능한 수준이 되도록 합니다.

실시간으로 플레이어 간 변경 사항을 동기화하기 위해 씬이 서드파티 서버로 데이터를 전송한다면, 변경 사항이 같은 렐름에 있는 플레이어들 사이에서만 동기화되도록 하는 것이 중요합니다. 하나의 렐름에 속한 모든 변경 사항은 다른 렐름의 변경 사항과 분리하여 처리해야 합니다. 그렇지 않으면 아무도 변경하지 않았는데도 플레이어들은 무언가가 기묘하게 변하는 것을 보게 됩니다.

각 플레이어의 렐름을 얻는 방법은 다음을 참고하세요: [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/)

## 멀티플레이어 지속성

플레이어가 들어올 때마다 새로 마운트되는 로컬 씬과 달리, 서드파티 서버를 사용하는 씬은 플레이어가 씬에 들어오고 나간 시점을 훨씬 넘어서도 수명이 이어집니다.

따라서 플레이어가 항상 같은 초기 상태의 씬을 만나게 되지는 않는다는 점을 고려해 경험을 설계해야 합니다. 씬에 가해진 변경 사항은 다른 플레이어가 발견할 수 있도록 남아 있으므로, 이러한 변경이 향후 플레이어의 경험에 원치 않는 방식으로 영향을 주지 않도록 해야 합니다.

### 상태 초기화

씬을 로드할 때는 기본 상태가 아니라 서버에 저장된 공유 정보를 바탕으로 구성되도록 하세요.

경우에 따라 씬에 일종의 초기화 버튼을 넣는 것이 타당할 수 있습니다. 초기화 버튼을 누르면 씬이 자연스럽게 재설정됩니다.

때때로 이는 씬 상태의 변수들을 기본값으로 되돌리는 것만을 의미합니다. 하지만 씬을 재설정하는 것은 서버 측에서 리스너 구독을 해제하고 루프를 중지하는 일까지 포함할 수 있습니다. 씬이 초기화될 때마다 빈 루프가 남아 있으면, 그것들이 계속 쌓여 씬 성능에 나쁜 영향을 미치게 됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/networking/third-party-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
