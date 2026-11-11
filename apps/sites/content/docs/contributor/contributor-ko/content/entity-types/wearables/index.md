# 착용 아이템

웨어러블은 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 플레이어가 착용하고 아바타를 커스터마이즈할 수 있는 아이템을 담고 있는

이 `웨어러블` 플레이어의 [프로필](/docs/contributor/contributor-ko/content/entity-types/profiles/) 현재 착용 중인 아이템에 대한 포인터 목록을 포함합니다.

모든 웨어러블은 *카테고리*이며, 아바타 몸체에서 차지하는 위치를 나타내고, 다른 카테고리를 숨기거나 대체할 수 있습니다(예: 긴 드레스는 상의로 착용될 수 있지만 다리를 완전히 가릴 수 있습니다).

18개의 웨어러블 카테고리가 있으며, 다양한 메타데이터 필드에서 참조됩니다: `몸체 형태`, `hair`, `eyes`, `눈썹`, `수염`, `입`, `상체`, `하체`, `발`, `모자`, `헬멧`, `마스크`, `티아라`, `머리 위쪽`, `귀걸이`, `안경`, `skin` 및 `손 착용`.

웨어러블 엔티티에는 서로 다른 몸체 형태에 대한 GLB 형식 파일이 포함됩니다.

### 포인터 <a href="#pointers" id="pointers"></a>

웨어러블에는 정확히 하나의 연결된 [포인터](/docs/contributor/contributor-ko/content/pointers/)가 있으며, 이는 [collection](/docs/contributor/contributor-ko/content/collections/) 에 속하는지, 그리고 그 안에서의 식별자가 무엇인지 나타냅니다.

웨어러블이 온체인 자산일 때, 포인터는 다음 형식의 URN입니다:

```
urn:decentraland:<blockchain>:collections-v2:<collection address>:<item index>
```

예를 들어, `포인터` 온체인 웨어러블의 배열은 다음과 같습니다:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... 다른 엔티티 속성들
}
```

웨어러블이 오프체인 자산일 경우, 예를 들어 새 아바타의 기본 웨어러블처럼, 다음 형식을 따릅니다:

```
urn:decentraland:off-chain:<collection>:<item name>
```

이 경우 컬렉션은 주소가 아니라 이름이며, 아이템 식별자 역시 인덱스가 아니라 이름입니다.

이 `포인터` 오프체인 웨어러블의 배열은 다음과 같습니다:

```json
{
  "pointers": [
    "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  // ... 다른 엔티티 속성들
}
```

### 메타데이터 필드

웨어러블은 기본 필드 대부분을 [이모트](/docs/contributor/contributor-ko/content/entity-types/emotes/)와 공유합니다. The `data` 속성에는 웨어러블별 정보가 들어 있습니다.

| 필드                  | 값                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------- |
| `id`                | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 웨어러블로 해석되며(또는 과거에 해석되던)    |
| `name`              | 의 표시 제목 [collection](/docs/contributor/contributor-ko/content/collections/).             |
| `description`       | 이 웨어러블에 대한 확장 설명.                                                                     |
| `이미지`               | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 의 그림.               |
| `썸네일`               | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 의 256x256 버전 `이미지`. |
| `희귀도`               | 다음 중 하나: `일반`, `드문`, `희귀`, `에픽`, `전설`, `이국적`, `신화` 또는 `고유`.                           |
| `i18n`              | 에 대한 번역 배열 `name` 필드.                                                                 |
| `collectionAddress` | 이 웨어러블을 포함하는 컬렉션의 이더리움 주소입니다.                                                         |
| `metrics`           | 애니메이션에 관한 유용한 몇 가지 측정값입니다(아래 참조).                                                     |
| `data`              | 이 웨어러블의 확장 메타데이터(아래 참조).                                                              |

일반적인 JSON 형식은 다음과 같습니다:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632b4e7226816a16139fb33:0",
  "name": "예쁜 드레스",
  "description": "착용하기 매우 좋음",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "드문",
  "i18n": [
    { "code": "es", "text": "예쁜 드레스" }
  ],
  "collectionAddress": "0x11a6879861f36cbad632b4e7226816a16139fb33",
  "metrics": {
    // 측정 객체(아래 참조).
  },
  "data": {
    // 확장 메타데이터(아래 참조).
  }
}
```

### 측정치

다음 `metadata.metrics` 객체에서 이 웨어러블과 함께 패키징된 모델에 대한 몇 가지 간단한 측정값을 찾을 수 있습니다. 예:

```json
{
  "entities": 1,
  "bodies": 12,
  "materials": 2,
  "meshes": 12,
  "textures": 2,
  "triangles": 3321
}
```

### 데이터

이 `metadata.data` 객체에는 웨어러블별 속성이 들어 있습니다.

| 필드      | 값                              |
| ------- | ------------------------------ |
| `카테고리`  | 위에 나열된 웨어러블 카테고리 중 하나.         |
| `표현`    | 서로 다른 몸체 형태에 연결된 파일 배열(아래 참조). |
| `숨깁니다`  | 이 웨어러블이 숨기는 카테고리 배열.           |
| `대체할 때` | 이 웨어러블이 완전히 대체하는 카테고리 배열.      |
| `tags`  | 이 웨어러블을 설명하는 문자열 레이블 배열.       |

#### 표현

representations 배열의 각 항목은 몸체 형태를 모델 모음과 연결하며, 각 카테고리에서 어떤 웨어러블이 표시되는지에 관한 규칙을 추가할 수 있습니다.

| 필드                 | 값                                                                                                            |
| ------------------ | ------------------------------------------------------------------------------------------------------------ |
| `bodyShapes`       | 다음의 배열 [포인터](/docs/contributor/contributor-ko/content/pointers/) 몸체 형태 엔티티에.                                    |
| `mainFile`         | 렌더링을 시작할 3D 모델 파일입니다.                                                                                        |
| `contents`         | 최상위 [`콘텐츠` 필드에 있는 파일 이름 배열](/docs/contributor/contributor-ko/content/entities/#properties)에서 사용됩니다. `mainFile`. |
| `overrideHides`    | 다음을 대체합니다 `숨깁니다` 이 표현에 대한 위 목록.                                                                              |
| `overrideReplaces` | 다음을 대체합니다 `대체할 때` 이 표현에 대한 위 목록.                                                                             |

다음은 일반적인 `표현` 항목입니다:

```json
{
  "bodyShapes": [
      "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  "mainFile": "male/190.glb",
  "contents": [
      "male/190.glb"
  ],
  "overrideHides": [ "lower_body" ],
  "overrideReplaces": []
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
