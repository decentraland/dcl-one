# 컬렉션

컬렉션은 다음으로 이루어진 그룹입니다 [웨어러블](/docs/contributor/contributor-ko/content/entity-types/wearables/) 및 [이모트](/docs/contributor/contributor-ko/content/entity-types/emotes/) 동일한 오프체인 네임스페이스 또는 온체인 컨트랙트에 정의됩니다.

그것들은 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 그 자체는 아니지만, 각 개별 아이템은 일반적으로 콘텐츠 서버에서 다음을 사용해 가져올 수 있습니다 [포인터](/docs/contributor/contributor-ko/content/pointers/).

### URN과 포인터 <a href="#pointers" id="pointers"></a>

컬렉션은 URN으로 식별되며, 이는 각 아이템 URN의 접두사입니다. URN에는 일반적으로 4가지 유형이 있습니다:

* `v1` 재단 승인 콘텐츠를 포함한 컬렉션(사용 중단됨).
* `v2` 커뮤니티 승인 콘텐츠를 포함한 컬렉션.
* `제3자` Decentraland 외부의 NFT와 연결된 아이템을 포함한 컬렉션.
* `오프체인` 새 아바타를 위한 기본 아이템이 주로 포함된 컬렉션.


> [!NOTE]
> 비록 `v1` 컬렉션은 더 이상 사용 중단되었고 생성되거나 업데이트되지 않지만, 여전히 유효하며 그 아이템은 사용할 수 있습니다.


#### 버전 1 및 2 컬렉션

이전의 `v1` 및 현재의 `v2` 컬렉션 네임스페이스는 모두 같은 형식의 URN을 가집니다:

```
urn:decentraland:<blockchain>:<collections-version>:<contract-address>
```

예를 들면:

```
# 사용 중단된 v1 공간의 컬렉션:
urn:decentraland:mainnet:collections-v1:DCL Test Masks

# v2 커뮤니티 승인 공간의 컬렉션:
urn:decentraland:matic:collections-v2:0x25a1d66891d44cdf7b8c45802489c1dea7aadf8b
```

만약 `:<id>` 세그먼트가 끝에 추가되면, URN은 다음이 됩니다 [포인터](/docs/contributor/contributor-ko/content/pointers/) 컬렉션 내부의 아이템을 가리키는 포인터가 됩니다. 다음을 참조하세요: [웨어러블 포인터](/docs/contributor/contributor-ko/content/entity-types/wearables/#pointers) 및 [이모트 포인터](/docs/contributor/contributor-ko/content/entity-types/emotes/#pointers) 이에 대한 자세한 정보는

#### 제3자 컬렉션

제3자 컬렉션은 다음으로도 알려져 있으며 [연결된 웨어러블](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md)일반 웨어러블과는 달리 Decentraland 외부의 기존 NFT와 연결되어 있습니다. 이를 통해 플레이어는 자신의 보유 NFT를 게임 내 아바타에 표시할 수 있습니다.

그 URN은 다음과 같습니다:

```
urn:decentraland:<blockchain>:collections-thirdparty:<third-party-id>:<collection-id>
```

이에 대한 충분한 정보는 다음에서 찾을 수 있습니다: [연결된 웨어러블](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md) 크리에이터 문서.

### 컬렉션 찾기

콘텐츠 서버는 다음을 사용하여 알려진 모든 컬렉션 목록을 제공할 수 있습니다: [`/lambdas/collections`](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/getCollections) 엔드포인트.

응답은 다음 속성을 가진 객체입니다: `collections` 속성으로, URN과 이름의 배열을 포함합니다. 예를 들면:

```json
{
  "collections": [
    {
      "id": "urn:decentraland:amoy:collections-v2:0xff5d4ebc6bc1ff7262cab42d3c693d953f4614d2",
      "name": "Winter clothes collection"
    },
    // ... 더 많이
  ]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/collections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
