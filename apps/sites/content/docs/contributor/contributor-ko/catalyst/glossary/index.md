# 용어집

#### 카탈리스트

[카탈리스트](https://github.com/decentraland/catalyst) 다음에 붙여진 이름입니다 **Decentraland 서비스 번들**, 각 Catalyst [노드](https://decentraland.github.io/catalyst-monitor) 플랫폼의 중추 역할을 하는 서비스 집합을 제공하며, 또한 공개 [API](https://decentraland.github.io/catalyst-api-specs/).

#### 부트스트랩

Catalyst가 시작되면, 노드는 하나의 `Bootstrapping` 다른 catalyst들에서 콘텐츠를 가져오는 동안 상태를 유지하며, 다른 것들을 따라잡을 때까지 그렇게 합니다. 그런 다음 다음 상태로 전환합니다 `Syncing` 그 상태에서 콘텐츠를 제공할 준비가 됩니다.

#### Catalyst 클라이언트

[라이브러리](https://github.com/decentraland/catalyst-client) Catalyst 서버와 상호작용하는 데 사용할 수 있습니다. 이를 통해 데이터 가져오기와 지정된 서버에 새 엔티티 배포가 모두 가능합니다.

#### 엔티티

데이터 유형, 다음으로 정의됨 [스키마](https://github.com/decentraland/common-schemas), 크기 때문에 블록체인에 저장할 수 없습니다. 현재의 일부 [지원되는 유형](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13) 은 다음과 같습니다:

* [웨어러블](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/wearable/wearable.ts)
* [이모트](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/emote/emote.ts)
* [씬](https://github.com/decentraland/common-schemas/blob/main/src/platform/scene/scene.ts)
* [프로필](https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts)
* [상점](https://github.com/decentraland/common-schemas/blob/main/src/dapps/store.ts)
* [코디](https://github.com/decentraland/common-schemas/blob/main/src/platform/outfits/outfits.ts)

#### 배포

콘텐츠 서버에서 생성되고 고유한 [EntityId](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L31). 배포는 어떤 [엔티티 유형](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13).

#### 실패한 배포

콘텐츠 서버에 배포된 엔티티 인스턴스로, 다음을 통과하지 못한 [콘텐츠 검증](https://adr.decentraland.org/adr/ADR-51). 검증은 다음에서 구현됩니다 [콘텐츠 검증기](https://github.com/decentraland/content-validator) 라이브러리.

#### 활성 엔티티

현재 포인터에 배포된 엔티티로, 따라서 모든 활성 엔티티는 Decentraland의 실시간 콘텐츠를 나타냅니다: 씬, 프로필, 웨어러블 등.

#### 포인터

포인터는 배포가 속하는 ID 또는 위치입니다. 그 형식은 다음에 따라 다릅니다 `엔티티 유형`, 예를 들면:

* **착용 아이템**: 포인터로 URN을 사용합니다 `urn:decentraland:ethereum:collections-v1:dg_atari_dillon_francis:gerald_patchwork_knit_sweater`
* **씬**: 포인터로 파셀 또는 파셀 배열을 사용합니다. 예를 들면 `--10,20` 또는 `[(0,0), (0,1), (0,-1)]`
* **프로필**: 포인터로 지갑 주소를 사용합니다 `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75`
* **상점**: 포인터로 URN을 사용합니다 `urn:decentraland:off-chain:marketplace-stores:{wallet}`
* **코디**: 포인터로 지갑 주소를 사용하며, 뒤에 `:outfits`, `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75:outfits`

#### URN

[URN](https://en.wikipedia.org/wiki/Uniform_Resource_Name) 는 Uniform Resource Name의 약자입니다. 이는 메타버스 프로젝트 전반에서 자산의 전역 고유 식별자로 사용됩니다.

* URN 해석기: <https://github.com/decentraland/urn-resolver>
* URN 네임스페이스: <https://github.com/common-metaverse/urn-namespaces>

#### 콘텐츠 서버

이 [콘텐츠 서버](https://github.com/decentraland/catalyst/tree/main/content) 엔티티 배포의 분산 저장을 관리하는 Catalyst 노드의 일부입니다. 현재 콘텐츠 서버에 저장된 모든 콘텐츠는 나머지 DAO Catalyst들과 동기화됩니다.

#### Lambdas

이 [Lambdas](https://github.com/decentraland/catalyst/tree/main/lambdas) Service는 콘텐츠 서버와 블록체인 사이의 조정 계층으로 작동하는 Catalyst 노드의 일부로, 콘텐츠를 정화하고 소유권을 검증하며 Catalyst 클라이언트가 정보를 조회하는 데 도움을 줍니다.

#### 동기화

콘텐츠 서버는 그 위에 콘텐츠를 배포할 수 있는 기능을 제공합니다. 또한 DAO의 다른 Catalyst에 배포된 모든 콘텐츠도 여기에서 사용할 수 있도록 보장합니다. 이를 해결하는 것을 **동기화**.

#### 콘텐츠 스냅샷

특정 유형의 모든 활성 엔티티 집합이 하나의 [콘텐츠 서버](#content-server). 이 콘텐츠 스냅샷은 Catalyst 노드 간 콘텐츠를 동기화하는 데 사용됩니다.

#### Realm

Decentraland에 연결되면 동일한 Realm에 연결된 Peer와만 상호작용할 수 있습니다. 친숙한 이름을 사용하는 Realm은 Hela나 Loki 같은 그리스 신 이름을 사용합니다. 각 Realm은 하나의 [Catalyst 노드](https://decentraland.github.io/catalyst-monitor/).

#### Peer

Decentraland에 연결된 사용자.

#### Lighthouse

이 [Lighthouse](https://github.com/decentraland/lighthouse) Catalyst 노드에서 호스팅되며 P2P 네트워크에서 사용자를 그룹화하는 데 사용되던 Communications Service의 이전 버전을 지칭하는 이름입니다. 주요 책임은 P2P 연결 후보를 결정하고 연결을 성립하기 위한 WebRTC 시그널링을 수행하는 것이었습니다. 이 로직의 대부분은 두 개의 외부 구성 요소를 통해 수행되었습니다: PeerJS Server(연결 [WebRTC](https://webrtc.org/) 피어)와 Archipelago(사용자의 위치를 받아 섬으로 그룹화함).

#### Backend for Frontend(일명 BFF)

이 [BFF](https://github.com/decentraland/explorer-bff) Service는 클라이언트 요구를 해결하고 기존 API를 깨뜨리지 않으면서 새로운 기능을 더 빠르게 개발할 수 있도록 만들었습니다. Catalyst Communication Service의 맥락에서는 P2P 시그널링을 관리하며, 따라서 Lighthouse를 대체하게 됩니다.

#### Catalyst 소유자

하나의 [Catalyst 소유자](https://github.com/decentraland/catalyst-owner) 는 Catalyst 노드를 운영하는 개인 또는 조직입니다. DAO 네트워크에 Catalyst 노드를 추가하려면 [제안](https://governance.decentraland.org/?type=catalyst) 을 제출해야 하며, DAO 노드는 신뢰할 수 있는 Decentraland 네트워크를 나타냅니다.

#### Archipelago 서비스

이 [Archipelago 서비스](https://github.com/decentraland/archipelago-service) 이전에는 Lighthouse가 사용하던 라이브러리였습니다. 이 서비스는 그 라이브러리와 같은 책임을 가지게 됩니다. 즉, 피어를 클러스터로 그룹화하여 서로 효율적으로 통신할 수 있게 하는 것입니다. 각 클러스터를 섬(island)이라고 합니다. 한편, 이 서비스는 사용 가능한 통신 전송 수단을 활용하고 다음의 [Catalyst 소유자](https://github.com/decentraland/catalyst-owner) 정의된 규칙에 따라 섬을 균형 있게 배치할 수 있어야 합니다. 예를 들어 Casino 씬의 섬에는 LiveKit를, Genesis Plaza에는 P2P를 사용하도록 하기 위해서입니다.

#### 섬

연결된 피어의 클러스터.

#### 통신 전송 수단

전송은 섬에 연결된 피어 간에 업데이트를 공유하는 데 사용되는 기술입니다.

#### P2P 전송

사용 [WebRTC](https://webrtc.org/) 피어 간 연결을 사용하며, 부하가 클라이언트 측에 있고 서버는 시그널링만 관리하므로 확장을 위한 추가 인프라가 필요하지 않습니다. 섬에 더 많은 피어가 추가될수록 지연 시간이 증가합니다. 클라이언트가 가질 수 있는 WebRTC 연결 수에는 제한이 있으므로, 더 큰 섬을 만들려면 피어 간 더 많은 간접 연결(점프)이 필요합니다.

#### LiveKit 전송

[라이브킷](https://livekit.io/) 는 WebRTC를 통한 확장 가능한 다중 사용자 회의를 제공하는 오픈 소스 프로젝트입니다. P2P 네트워크를 사용하는 대신, 피어는 메시지 릴레이와 통신의 다양한 품질 측면을 관리하는 Selective Forwarding Unit(SFU)에 연결됩니다. SFU는 지정된 씬에서 군중 간의 고성능/고품질 통신을 제공하기 위해 추가로 필요한 인프라입니다.

#### WebSocket 전송

이 [WebSocket 전송](https://github.com/decentraland/ws-room-service) 는 LiveKit보다 가벼우며, WebSocket 룸을 통해 메시지 중계를 관리하는 서버도 필요합니다. 룸은 섬과 같습니다. 이 전송은 P2P 섬보다 지연이 더 낮고 성능이 더 좋지만, 추가 인프라도 필요합니다.

#### NATS

[NATS](https://nats.io/) 는 서비스 간 데이터 교환과 통신을 가능하게 하는 메시지 브로커입니다. Catalyst Server의 맥락에서는 일부 내부 서비스와 통신하는 데 사용됩니다.

#### DAO

DAO는 "Decentralized Autonomous Organization"의 약자입니다. Decentraland DAO는 Decentraland를 구성하는 가장 중요한 스마트 계약과 자산인 LAND Contract, Estates Contract, Wearables, Content Servers, Marketplace를 소유합니다. 또한 상당한 양의 MANA도 보유하고 있어 진정한 자율성을 가질 수 있으며 Decentraland 전반의 다양한 운영과 이니셔티브를 지원할 수 있습니다.

<https://dao.decentraland.org/en/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/catalyst/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
