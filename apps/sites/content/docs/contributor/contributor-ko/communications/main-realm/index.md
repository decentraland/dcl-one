# MAIN 렐름

Decentraland는 통신 서비스를 제공하는 Catalyst의 수만큼 realm을 가지고 있지만, Catalysts 네트워크 밖에 존재하는 MAIN이라는 고유한 realm이 하나 있습니다. MAIN realm은 사용자 연결 프로세스를 간소화하도록 특별히 설계되어 사용자가 더 쉽게 상호작용하고 만날 수 있게 합니다. 이는 Catalyst 노드의 통신 서비스를 대체할 수 있는 서비스 집합으로 구성됩니다. 이 realm은 사용자가 Decentraland에 로그인할 때의 기본 목적지 역할을 합니다.

<img src="/docs/_remote/d01dbf731e2a22a53eeb9e6a.webp" alt="" width="800">

이 [realm 제공자](https://github.com/decentraland/realm-provider) 서비스는 ...을 노출합니다 [/about](https://realm-provider.decentraland.org/main/about) realm 설명이 포함된 엔드포인트를 다음에 따라 [ADR-110: Realm 설명](https://adr.decentraland.org/adr/ADR-110). 이를 통해 Catalyst 노드의 서비스 중 하나로 알려진 콘텐츠 관리 네트워크에 연결을 설정하는 데 필요한 정보를 제공하며, MAIN realm 통신 서비스의 연결 세부 정보도 제공합니다.

MAIN 통신 서비스는 다음을 통해 접근됩니다 [WebSocket 커넥터](https://github.com/decentraland/archipelago-workers). 이 경량 구성 요소는 나머지 백엔드 구성 요소와의 상호작용을 가능하게 하며, 증가하는 사용자 부하를 수용하기 위해 추가 인스턴스로 쉽게 확장할 수 있어 더 많은 활성 연결을 효율적으로 관리합니다. WebSocket 연결을 통해 클라이언트는 LiveKit Cloud와의 연결을 설정하는 데 필요한 데이터와 함께 `islandId` 는 Archipelago Service에 의해 할당됩니다. Archipelago Service는 MAIN realm과 Catalyst 노드 통신 서비스 모두에서 동일한 책임을 집니다. 이는 사용자 위치의 처리와 이후 개별 섬으로의 배정을 효과적으로 관리하며, 각 섬은 Livekit 룸에 해당하므로 가상 세계 내에서의 공간적 위치에 따라 사용자 클러스터를 효율적으로 조직합니다. Archipelago Service에 대한 더 자세한 내용은 다음을 참조할 수 있습니다. [Archipelago](/docs/contributor/contributor-ko/communications/archipelago/) 페이지.

[NATS](https://nats.io/) 는 원활한 통신을 촉진하는 브로드캐스팅 시스템으로 기능합니다. 동일한 Archipelago Service와 통신하는 여러 WebSocket Connector 인스턴스를 지원할 수 있습니다. 또한 Stats 서비스가 Archipelago에서 위치 및 섬 그룹화와 같은 사용자 통계를 수집할 수 있게 하여, 이 데이터를 외부에서 활용할 수 있도록 합니다.

Archipelago stats 서비스는 Catalyst 노드의 Archipelago 서비스와 동일한 API 구현을 공유합니다. 다음 링크에서 API를 살펴볼 수 있습니다: [Catalyst API 명세](https://decentraland.github.io/catalyst-api-specs/#tag/Archipelago). 이 서비스는 다음 URL을 통해 접근할 수 있습니다 `https://archipelago-stats.decentraland.org/` 사용 가능한 엔드포인트는 다음 아래에 있습니다 `/comms/` API 명세에 설명된 대로, 예: `https://archipelago-stats.decentraland.org/comms/peers`.

이 [라이브킷](/docs/contributor/contributor-ko/communications/transport-types/livekit/) Cloud 플랫폼은 WebRTC를 통해 사용자 정보를 교환하는 복잡한 작업을 관리하며, 동일한 목적을 위해 대부분의 Catalyst 노드가 공유하는 기본 클라우드 플랫폼입니다.

MAIN realm에서 비롯된 핵심 개념은 서로 다른 사용자가 콘텐츠 관리 목적으로 어떤 Catalyst 노드에든 유연하게 연결할 수 있다는 것입니다(씬, 웨어러블, 이모트 다운로드 또는 프로필 업데이트 전송). 하지만 특정 콘텐츠 관리 노드를 선택하더라도, 통신 서비스에 대한 연결은 **할 수** 동일할 수 있습니다. 그 결과, 사용자가 서로 다른 Catalyst 노드를 콘텐츠 서버로 사용하더라도 서로 가까운 씬 좌표에 있다면 서로 상호작용할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/main-realm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
