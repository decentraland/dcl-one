> Decentraland에서 만들 수 있는 다양한 프로젝트 유형.

# 개요

Decentraland를 위해 만들 수 있는 다양한 유형의 콘텐츠가 있습니다.

![](/docs/_remote/0a4a8ae5cf4dbf5eb00b75db.webp)

## 씬

Decentraland 씬은 Genesis City의 Parcel 또는 WORLDs에 호스팅할 수 있습니다.

### LAND에 게시

Parcel에 게시된 씬은 Decentraland의 오픈 월드 내 특정 좌표에서 찾을 수 있습니다.

이러한 씬은 LAND 또는 Estate 토큰에 연결되어 있습니다. 각 Parcel은 16x16미터를 차지합니다. 인접한 여러 Parcel을 하나의 씬에 사용할 수 있으며, 경계가 맞닿아 있기만 하면 어떤 모양으로도 배치할 수 있습니다.

LAND 토큰은 다음에서 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/lands). 이 토큰은 수량이 한정되어 있으며, Genesis City의 지도를 덮고 있습니다.

LAND에 게시된 씬은 플레이어가 주변 콘텐츠를 방문하거나 탐색하는 동안 우연히 마주칠 수 있어 더 쉽게 발견됩니다.

LAND에 게시된 씬은 씬 내 각 Parcel당 최대 15MB의 공간을 사용할 수 있습니다. Parcel 수가 많을수록 사용할 수 있는 공간도 더 많아집니다. 이는 플레이어가 동시에 많은 주변 씬을 경험할 수 있으므로, 플레이어의 CPU가 과부하되는 것을 방지하기 위한 것입니다. 자세한 내용은 [크기 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/).

### WORLDs에 게시

World에 게시된 씬은 링크를 통해서만 접근할 수 있습니다.

이러한 씬은 NAME 토큰에 연결되어 있습니다. NAME 토큰은 다음에서 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/names/claim). 아직 선점되지 않은 이름이라면 원하는 이름을 무엇이든 등록할 수 있습니다. 또는 [ENS 도메인](https://ens.domains) 를 사용해 World를 만들 수 있습니다.

World에 게시된 씬은 Parcel 제한이 300x300입니다. LAND Parcel에 게시된 씬과 동일한 [크기 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 당 공간 제한이 적용되지만, 추가 비용 없이 씬에 더 많은 Parcel을 추가할 수 있습니다.

Multi-Scene WORLD인 경우, 서로 다른 Collaborator(주소)가 World Owner(해당 World NAME을 등록한 주소)가 정의한 WORLD의 특정 Parcel에 대한 게시 권한을 가질 수 있습니다. 또한 World에 대한 접근은 **공개** 또는 **비공개**일 수 있습니다. 자세한 정보는 [World 관리](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#managing-worlds)

World는 **공개** 또는 **비공개**

#### 크기 제한

World에 업로드할 수 있는 최대 파일 크기는 Decentraland NAME을 사용하는지 ENS 도메인을 사용하는지에 따라 달라집니다.

**Decentraland NAME의 World**

Decentraland NAME 보유자는 지갑 보유량에 따라 Foundation Worlds Content Server 내에서 동적 저장 용량을 이용할 수 있습니다. 다음 규칙이 이 할당을 관리합니다:

* 보유한 각 Decentraland NAME마다 100MB의 저장 용량이 부여됩니다(그리고 World도 함께 제공됩니다).
* 보유한 각 Decentraland LAND Parcel마다 추가로 100MB의 저장 용량이 부여됩니다.
* 지갑에 보유한 MANA 2,000개마다 추가로 100MB의 저장 용량이 부여됩니다.

Foundation Worlds Content Server의 공간은 사용자가 원하는 만큼 큰 씬을 호스팅하는 데 사용할 수 있으며, 사용자가 소유한 Decentraland NAME와 그에 따른 Decentraland 자산 전체에서 부여된 공간을 함께 활용합니다. 예를 들어, 여러 World(여러 NAME 소유로 인해 부여됨)와 총 500MB의 저장 용량을 보유한 사용자는 200MB 씬 파일이 있는 World 하나, 200MB 씬 파일이 있는 World 하나, 100MB 씬 파일이 있는 World 하나를 배포할 수 있습니다. 또는 300MB 씬 파일이 있는 World 하나와 200MB 씬 파일이 있는 World 하나를 배포할 수도 있습니다.

저장 용량 예산 중 얼마나 사용되었는지, 그리고 얼마나 남았는지는 두 곳에서 확인할 수 있습니다:

* 해당 **관리** Creator Hub의 섹션. 다음을 클릭하세요: **세부 정보 보기** MANA, LAND, NAME 보유량이 어떻게 합산되는지 내역을 보려면
* 해당 **Worlds** 탭에서 [빌더](https://decentraland.org/builder/worlds)는 각 World가 사용 중인 공간도 보여줍니다.

**ENS 도메인의 World**

반면 ENS 도메인에서 부여된 World는 사용자의 다른 Decentraland 보유량과 관계없이 World당 최대 씬 파일 크기가 고정적으로 36MB입니다. ENS 도메인에서 World를 보유한 사용자는 추가 MANA나 LAND를 구매하더라도 ENS World의 씬 크기 제한을 늘릴 수 없습니다.

하지만 ENS 도메인 소유로 부여되는 World는 Decentraland가 제공하는 창작의 자유를 실현하는 완벽한 첫걸음이 됩니다.

#### LAND 씬과의 차이점

World는 LAND 씬보다 여러 가지 장점이 있습니다:

* LAND를 소유할 필요 없음 - 비용 없이 원하는 만큼 Parcel을 씬에 추가할 수 있음(최대 300x300)
* 링크를 통한 접근 - 플레이어가 URL로 직접 World를 방문할 수 있음
* 더 많은 저장 용량 - 특히 Decentraland NAME 보유자에게 유리
* 접근 제어 - 특정 플레이어만 World에 접근하도록 허용할 수 있음

참조 [Worlds](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 자세한 내용은 다음을 확인하거나, 다음에 대해 알아보세요 [World에 게시하기](/docs/creator/content-creator-ko/sdk7/publishing/publishing/#publishing-to-worlds) 및 [World 설정 구성하기](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#world-configuration).

## 글로벌 씬

글로벌 씬은 이미 존재하는 Decentraland의 풍경을 변화시켜 상호작용과 게임플레이의 층위를 더할 수 있습니다. 이들은 특정 LAND Parcel이나 특정 World에서만 실행되도록 제한되지 않는 씬입니다. 플레이어는 어디를 가든 이들을 함께 가지고 다닙니다.

### 스마트 웨어러블

스마트 웨어러블은 Wearable 토큰에 연결되어 있습니다. 이는 NFT로 판매되며 다음에서 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

스마트 웨어러블은 플레이어가 연결된 웨어러블 아이템을 착용할 때마다 활성화됩니다. 플레이어가 아이템을 벗으면 비활성화되며, UI를 통해 글로벌 씬이 직접 꺼지도록 할 수도 있습니다. 또한 플레이어가 서 있는 씬은 해당 씬에서 활성화된 모든 스마트 웨어러블을 일시 중단할 수 있습니다.

* 에 대해 모든 것을 알아보세요 [웨어러블 만들기](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/).
* 다음에 대해 알아보기 [스마트 웨어러블](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
