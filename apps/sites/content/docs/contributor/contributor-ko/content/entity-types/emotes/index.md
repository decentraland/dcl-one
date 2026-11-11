# 이모트

이모트는 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 플레이어의 아바타용 애니메이션을 담는

여러 신체 형태에 대한 GLB 형식의 파일을 포함합니다.

### 포인터 <a href="#pointers" id="pointers"></a>

이모트에는 정확히 하나의 관련 [포인터](/docs/contributor/contributor-ko/content/pointers/), 이는 이모트가 속한 컬렉션과 그 안의 인덱스를 나타냅니다. 이들은 다음과 같은 형식의 URN입니다:

```
urn:decentraland:matic:collections-v2:<collection address>:<item index>
```

예를 들면, `포인터` 이모트의 배열은 다음과 같습니다:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... 다른 엔티티 속성들
}
```

### 메타데이터 필드

이모트는 대부분의 기본 필드를 다음과 공유합니다 [웨어러블](/docs/contributor/contributor-ko/content/entity-types/wearables/). `emoteDataADR74` 속성에는 이모트별 정보가 들어 있습니다.

| 필드                  | 값                                                                                              |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `id`                | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 이모트를 가리키는(또는 과거에 가리켰던).             |
| `name`              | 이모트의 표시 제목은 [collection](/docs/contributor/contributor-ko/content/collections/).                  |
| `description`       | 이모트에 대한 확장 설명.                                                                                 |
| `이미지`               | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 이 이모트의 그림이 포함된.              |
| `썸네일`               | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 256x256 버전의 `이미지`.           |
| `희귀도`               | 다음 중 하나: `일반`, `비일반`, `희귀`, `에픽`, `전설`, `이국적`, `신화` 또는 `고유`.                                   |
| `i18n`              | 다음에 대한 번역 배열 `name` 필드.                                                                        |
| `collectionAddress` | 이 이모트를 포함하는 컬렉션의 이더리움 주소입니다.                                                                   |
| `metrics`           | 애니메이션에 대한 유용한 측정값입니다(아래 참조).                                                                   |
| `emoteDataADR74`    | 이 이모트에 대한 확장 메타데이터로, 다음에 정의되어 있습니다: [ADR-74](https://adr.decentraland.org/adr/ADR-74) (아래 참조). |

일반적인 JSON은 다음과 같습니다:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0",
  "name": "재미있는 댄스",
  "description": "챔피언처럼 움직여 보세요",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "희귀",
  "i18n": [
    { "code": "es", "text": "우아한 춤" }
  ],
  "collectionAddress": "0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5",
  "metrics": {
    // 측정 객체(아래 참조).
  },
  "emoteDataADR74": {
    // ADR-74에 정의된 확장 메타데이터(아래 참조).
  }
}
```

### 메트릭

다음 `metadata.metrics` 객체에는 이 이모트와 함께 패키징된 애니메이션에 대한 몇 가지 간단한 측정값이 들어 있습니다. 예:

```json
{
  "triangles": 0,
  "materials": 0,
  "textures": 0,
  "meshes": 0,
  "bodies": 0,
  "entities": 1
}
```

### 이모트 데이터 ADR-74

이 객체에는 World Explorers(또는 다른 그래픽 애플리케이션)가 이 이모트로 모델을 애니메이션하는 데 필요한 필드가 포함되어 있습니다.

| 필드      | 값                                                           |
| ------- | ----------------------------------------------------------- |
| `카테고리`  | 다음 중 하나: `댄스`, `스턴트`, `인사`, `재미`, `포즈`, `반응`, `공포` 또는 `기타`. |
| `표현 항목` | 서로 다른 신체 형태에 연결된 애니메이션 파일 배열입니다.                            |
| `tags`  | 이 이모트를 설명하는 문자열 태그 배열입니다.                                   |
| `루프`    | `true` 애니메이션이 끝난 뒤 반복되어야 하는 경우.                             |

명확성을 위해 일부 JSON을 보겠습니다:

```json
{
  "category": "댄스",
  "representations": [
    // 서로 다른 신체 형태에 대한 애니메이션 정보(아래 참조).
  ],
  "tags": [ "카니발", "댄스" ],
  "loop": true
}
```

#### 표현 항목

다음의 각 항목은 `metadata.emoteDataADR74.representations` 필드는 각 신체 형태에 맞는 애니메이션 파일을 정의합니다.

| 필드           | 값                                                                            |
| ------------ | ---------------------------------------------------------------------------- |
| `bodyShapes` | 적용 가능한 신체 형태 배열 [포인터](/docs/contributor/contributor-ko/content/pointers/).      |
| `mainFile`   | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 주 GLB 파일용. |
| `contents`   | 다음을 포함한 모든 파일: `mainFile` 및 참조할 수 있는 추가 파일들.                                 |

예를 들면, `표현 항목` 항목:

```json
{
  "bodyShapes": [ "urn:decentraland:off-chain:base-avatars:BaseMale" ],
  "mainFile": "male/funnydance.glb",
  "contents": [
    "male/funnydance.glb"
  ]
}
```

일반적으로 두 개의 representation 항목을 찾게 되며, 하나는 `BaseMale` 그리고 하나는 `BaseFemale`.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
