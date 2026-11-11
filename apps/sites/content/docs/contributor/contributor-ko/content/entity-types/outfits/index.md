# 의상

Outfits는 플레이어가 저장한 아웃핏입니다. 아웃핏은 체형, 피부, 머리카락, 눈, 착용 아이템 설정으로 구성됩니다.

### 포인터 <a href="#pointers" id="pointers"></a>

코디 [포인터](/docs/contributor/contributor-ko/content/pointers/) 소유자의 이더리움 주소에 " 접미사가 붙은 것입니다.`:outfits`" 접미사입니다. 예를 들면:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df:outfits
```

### 메타데이터 필드

| 필드                   | 값                                                          |
| -------------------- | ---------------------------------------------------------- |
| `아웃핏`                | 각 아웃핏에 대한 설명 배열입니다(아래 참조).                                 |
| `namesForExtraSlots` | 추가 슬롯 검증에 사용되는 보유 이름 배열입니다. 보유한 이름마다 최대 5개까지 추가 슬롯이 부여됩니다. |

### 코디

의 `metadata.outfits[]` 사용자의 아웃핏을 나타내며, 여러 속성을 가집니다:

| 필드       | 값               |
| -------- | --------------- |
| `slot`   | 저장된 아웃핏의 슬롯 번호. |
| `outfit` | 저장된 아웃핏(아래 참조). |

예시는 다음과 같습니다:

```json
{
  "slot": 1,
  "outfit": {
    // 아래 참조
  }
}
```

#### 아웃핏 필드

이 `metadata.outfits[].outfit` 필드에는 플레이어의 아웃핏을 저장하고 렌더링하는 데 필요한 모든 정보가 들어 있습니다.

| 필드          | 값                                                                                                     |
| ----------- | ----------------------------------------------------------------------------------------------------- |
| `bodyShape` | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 아바타의 체형 엔티티에 대한 것입니다.                        |
| `eyes`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 눈을 위한 객체입니다.                                             |
| `hair`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 머리카락을 위한 객체입니다.                                          |
| `skin`      | 다음과 같은 객체입니다: `color` 형식의 `{ r, g, b }` 아바타의 피부를 위한 객체입니다.                                            |
| `웨어러블`      | 다음의 배열 [웨어러블 포인터](/docs/contributor/contributor-ko/content/entity-types/wearables/#pointers) 아바타가 사용 중인. |

예를 들면:

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
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


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/outfits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
