# 프로필

프로필은 플레이어의 기본 설명으로, 인게임 이름과 아바타와 같은 정보를 포함합니다.

콘텐츠 서버에서 일반적으로 제공됩니다 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/), 비록 월드 탐험가들은 보통 통신 시스템을 활용해 최신 버전을 즉석에서 가져옵니다.

이 시스템은 동일한 소유자를 가진 여러 정체성을 허용하도록 준비되어 있으며, 이들은 모두 엔티티 매니페스트에 아바타로 포함됩니다. 하지만 실제로는 대부분의 플레이어가 하나만 가지고 있습니다.

### 포인터 <a href="#pointers" id="pointers"></a>

프로필 [포인터](/docs/contributor/contributor-ko/content/pointers/) 접두사나 접미사 없이, 소유자의 이더리움 주소입니다. 예:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df
```

### 메타데이터 필드

| 필드    | 값                               |
| ----- | ------------------------------- |
| `아바타` | 소유자의 각 아바타에 대한 설명 배열입니다(아래 참조). |

### 아바타

각 아바타는 `metadata.avatars[]` 동일한 소유자를 가진 정체성을 나타내며, 클라이언트가 프로필을 표시하고, 인게임에서 아바타를 렌더링하고, 소유자에게 연락할 수 있도록 하는 여러 속성을 가집니다.

| 필드               | 값                                                                                  |
| ---------------- | ---------------------------------------------------------------------------------- |
| `userId`         | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 프로필로 연결되는(또는 과거에 연결되던). |
| `name`           | 이 플레이어의 표시 이름입니다.                                                                  |
| `email`          | (선택 사항) 이 플레이어의 이메일 주소입니다.                                                         |
| `description`    | (선택 사항) 플레이어가 자신을 설명하기 위해 선택한 텍스트입니다.                                              |
| `ethAddress`     | 이 플레이어의 이더리움 주소(현재는 해당 `userId`).                                                  |
| `hasClaimedName` | 이 `name` 필드가 청구된 ENS 이름인지 여부입니다.                                                   |
| `unclaimedName`  | ENS 이름이 없는 사용자(예: 게스트)를 위한 임시 이름입니다.                                               |
| `tutorialStep`   | 이 플레이어의 튜토리얼 진행 상태입니다.                                                             |
| `avatar`         | 플레이어의 인게임 아바타 속성입니다(아래 참조).                                                        |

예시는 다음과 같습니다:

```json
{
  "userId": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "name": "Bob",
  "email": "",
  "hasClaimedName": false,
  "description": "",
  "ethAddress": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "avatar": {
    // 아래 참조
  }
}
```

#### 아바타 필드

이 `metadata.avatars[].avatar` 필드에는 월드 탐험가(또는 독립형 아바타 편집기 같은 다른 클라이언트)가 플레이어를 렌더링하는 데 필요한 모든 정보가 들어 있습니다.

| 필드          | 값                                                                                                     |
| ----------- | ----------------------------------------------------------------------------------------------------- |
| `bodyShape` | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 아바타의 체형 엔티티에 대한 것입니다.                        |
| `스냅샷`       | 이 아바타의 이미지를 담은 객체입니다(아래 참조).                                                                          |
| `eyes`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 눈을 위한 객체입니다.                                             |
| `hair`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 머리카락을 위한 객체입니다.                                          |
| `skin`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 피부를 위한 객체입니다.                                            |
| `웨어러블`      | 다음의 배열 [웨어러블 포인터](/docs/contributor/contributor-ko/content/entity-types/wearables/#pointers) 아바타가 사용 중인. |

예를 들면:

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
  "snapshots": {
    // 아래 참조
  },
  "eyes": {
    "color": { "r": 0.37109375, "g": 0.22265625, "b": 0.1953125 }
  },
  "hair": {
    "color": { "r": 0.234375, "g": 0.12890625, "b": 0.04296875 }
  },
  "skin": {
    "color": { "r": 0.80078125, "g": 0.609375, "b": 0.46484375 }
  },
  "wearables": [
    "urn:decentraland:off-chain:base-avatars:eyes_00",
    "urn:decentraland:off-chain:base-avatars:eyebrows_00",
    "urn:decentraland:off-chain:base-avatars:mouth_00",
    "urn:decentraland:off-chain:base-avatars:m_mountainshoes.glb",
    "urn:decentraland:off-chain:base-avatars:sport_jacket",
    "urn:decentraland:off-chain:base-avatars:brown_pants",
    "urn:decentraland:off-chain:base-avatars:aviatorstyle",
    "urn:decentraland:off-chain:base-avatars:casual_hair_03",
    "urn:decentraland:off-chain:base-avatars:Mustache_Short_Beard"
  ]
}
```

#### 스냅샷

이 `metadata.avatars[].avatar.snapshots` 필드에는 [파일 식별자가](/docs/contributor/contributor-ko/content/file-system/#identifiers) 이미지용으로 ( [`콘텐츠` 최상위 필드](/docs/contributor/contributor-ko/content/entities/#properties))에 포함됩니다. 각 속성은 이미지 종류이며, 현재 두 가지가 있습니다: `face256` (썸네일) 및 `body` (전체 크기 버전).

예를 들면:

```json
{
  "face256": "bafkreidvjwc3gqqdqk646rqnxnqzpsyjsjafqfc76zenfgjnm2ahbvrtsd",
  "body": "bafkreigclygt6vzzu7myzg3weifapb3a5uec245lzndg4f53y4vfb3cmwm"
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/profiles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
