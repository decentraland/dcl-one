# 씬

씬은 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 다양한 Decentraland의 여러 부분에서 동작을 생성하는 엔티티입니다. 여러 랜드 파셀에 걸칠 수 있으며, World Explorer에서 그 안을 걸어 다니는 플레이어는 씬에서 제공한 특수 코드를 실행하게 됩니다.

완전한 실제 예시는 다음에서 찾을 수 있습니다. [이 씬의 매니페스트](https://peer.decentraland.org/content/contents/bafkreidihplrc5cxarjkji2enmzhicpjoa2vrlqyyffnholmxb3o2xft3u) Foundation의 콘텐츠 서버에서.

### 포인터 <a href="#pointers" id="pointers"></a>

씬에는 하나 이상의 관련 [포인터](/docs/contributor/contributor-ko/content/pointers/) 파셀 좌표 문자열 형태의 `"0,0"`가 있습니다. 씬을 실행하는 모든 파셀은 엔티티 매니페스트의 [`포인터` 배열](/docs/contributor/contributor-ko/content/entities/#properties) 에 나열되며, 이러한 모든 포인터는 콘텐츠 서버에서 해당 씬으로 해석됩니다.

다시 말해, 파셀의 x/y 좌표를 선택하고 콘텐츠 서버에 다음에 대해 질의하여 실행 중인 씬을 찾을 수 있습니다. `"<x>,<y>"` 포인터입니다. 플레이어가 월드를 탐색할 때 World Explorers가 씬의 동작을 활성화할지 결정하는 방식이 바로 이것입니다.

전형적인 `포인터` 배열은 다음과 같습니다:

```json
{
  "pointers": [
    "-113,-134",
    "-113,-133",
    "-113,-132",
  ],
  // ... 다른 엔티티 속성들
}
```

이 데이터는 아래에 자세히 설명된 `metadata.scene.parcels` 필드에서도 사용할 수 있습니다. 매니페스트에 씬 전용 객체만 저장하고 싶을 경우를 위해서입니다.

### 메타데이터 필드

다음에 대한 몇 가지 특별한 속성이 있습니다. [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 형식의 `씬` 다음에 위치한 `메타데이터` 최상위 필드.

| 필드                    | 값                                                                                               |
| --------------------- | ----------------------------------------------------------------------------------------------- |
| `owner`               | 씬의 유지관리자에 대한 정보입니다.                                                                             |
| `main`                | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 이 씬의 मुख्य JavaScript 파일에 대한. |
| `tags`                | 이 씬을 설명하는 문자열 레이블의 배열입니다.                                                                       |
| `display`             | 플레이어에게 보여주기 위한 씬 정보입니다(아래 참조).                                                                  |
| `씬`                   | 이 씬이 활성화된 파셀과 중심 위치입니다(아래 참조).                                                                  |
| `contact`             | 씬의 제작자 또는 유지관리자에게 연락할 수 있는 이름과 이메일 주소입니다(아래 참조).                                                |
| `spawnPoints`         | 플레이어가 이 씬에 진입할 때의 위치와 카메라 각도입니다(아래 참조).                                                         |
| `requiredPermissions` | World Explorer가 특정 작업을 허용하기 전에 플레이어에게 요청할 권장 권한입니다(아래 참조).                                      |
| `featureToggles`      | 기능 플래그 설정                                                                                       |


> [!NOTE]
> 엔티티와 그 메타데이터에는 사용자 정의 필드가 허용된다는 점을 기억하세요. 여기 나열되지 않은 속성도 있을 수 있습니다.


#### 표시

에서 `metadata.display`에는 플레이어가 씬에 들어가기 전에 씬에 대해 알리는 데 유용한 속성이 있습니다.

| 필드                | 값                                                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------------------------- |
| `title`           | 이 씬의 표시 이름입니다.                                                                                             |
| `description`     | 이 씬에 대한 확장 설명입니다.                                                                                          |
| `favicon`         | *더 이상 사용되지 않음* 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 이 씬이 활성화될 때 표시되는 아이콘에 대한. |
| `navmapThumbnail` | 이 [내부 파일명](/docs/contributor/contributor-ko/content/entities/#files) 월드 맵용 이 씬의 썸네일에 대한.                      |

예시는 다음과 같습니다:

```json
{
  "title": "my_cool_scene",
  "favicon": "favicon.png",
  "navmapThumbnail": "thumbnail.png"
}
```

#### 씬

이 `metadata.scene` 속성은 월드 맵에서 이 씬의 위치를 설명하는 객체입니다.

| 필드        | 값                        |
| --------- | ------------------------ |
| `parcels` | 이 씬을 실행하는 파셀 포인터의 배열입니다. |
| `base`    | 이 씬의 원점에 대한 파셀 포인터입니다.   |

실제로는 다음과 같습니다:

```json
{
  "parcels": [
    "17,-112", 
    "17,-113"
  ],
  "base": "17,-112"
}
```

이 `base` 필드는 항상 `parcels` 배열에 포함됩니다.

#### 연락처

이 `metadata.contact` 객체는 씬 소유자 또는 유지관리자의 연락처 정보를 담고 있습니다.

| 필드      | 값                      |
| ------- | ---------------------- |
| `name`  | 이 연락처의 이름입니다.          |
| `email` | (선택 사항) 연락할 이메일 주소입니다. |

JSON에서는:

```json
{
  "name": "cool_scene_maker",
  "email": "hello@decentraland.org"
}
```

동안 `name` 필드는 모든 씬에 채워져 있지만, 실제로는 `email` 필드가 비어 있는 문자열인 경우가 많습니다. 이는 씬 제작자가 선택한 개인정보 보호 방식입니다.

#### 스폰 포인트

이 `metadata.spawnPoints` 필드는 플레이어가 씬에 들어왔을 때 어디에 나타나야 하는지, 그리고 처음 어디를 바라봐야 하는지를 결정하는 포인트와 카메라 방향의 배열을 정의합니다. 정의된 스폰 포인트가 최소 하나는 있어야 합니다.

배열의 각 항목에는 여러 속성이 있습니다:

| 필드             | 값                                              |
| -------------- | ---------------------------------------------- |
| `name`         | 이 스폰 포인트를 식별하기 위한 레이블입니다.                      |
| `position`     | 다음의 시작 위치 `{ x, y, z }` 부동소수 좌표를 가진 객체입니다.     |
| `cameraTarget` | 다음의 시작 카메라 방향 `{ x, y, z }` 부동소수 좌표를 가진 객체입니다. |
| `default`      | 다른 지정이 없을 때 이 스폰 포인트가 사용되는지 여부입니다.             |

예를 들면:

```json
[
  {
    "name": "spawn1",
    "position": { "x": 10.02, "y": 5.27, "z": 15.23  },
    "cameraTarget": { "x": 10.02, "y": 6.27, "z": 31.23 },
    "default": true
  }
]
```


> [!NOTE]
> **📔 참고**: 스폰 영역과 카메라 타겟은 Creator Hub를 사용해 설정할 수 있습니다. 자세한 내용은 [스폰 영역](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/spawn-areas.md) 문서를 참조하세요.


#### 필수 권한 <a href="#permissions" id="permissions"></a>

에서 `metadata.requiredPermissions` World Explorer가 특정 작업을 허용하기 전에 플레이어에게 요청해야 할 권한을 선언하는 잘 알려진 문자열 배열을 찾을 수 있습니다. 이는 Explorer에 해당하며, 다른 목적을 위해 만들어진 클라이언트는 이를 무시할 수 있습니다.

현재 지원되는 권한 집합은 다음과 같습니다:

| 권한                                  | 목적                                         |
| ----------------------------------- | ------------------------------------------ |
| `USE_FETCH`                         | 씬이 외부 HTTP 요청을 수행하도록 허용합니다.                |
| `USE_WEBSOCKET`                     | 씬이 Websocket API를 사용하여 외부 연결을 설정하도록 허용합니다. |
| `OPEN_EXTERNAL_LINK`                | 씬이 URL을 여는 것을 허용합니다(브라우저 탭 또는 웹 뷰에서).      |
| `USE_WEB3_API`                      | 씬이 지갑과 통신하도록 허용합니다.                        |
| `ALLOW_TO_TRIGGER_AVATAR_EMOTE`     | 씬이 이모트로 플레이어의 아바타를 애니메이션하도록 허용합니다.         |
| `ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE` | 씬이 플레이어의 위치를 변경하도록 허용합니다.                  |

#### 기능 토글

이 `metadata.featureToggles` 필드는 씬이 World Explorer의 특정 기능을 활성화하거나 비활성화해야 하는지 나타낼 수 있게 해줍니다.

필드에는 다음 형식의 객체가 들어 있습니다 `{ [featureName]: 'enabled' | 'disabled' }`와 같습니다:

```json
{
  "voiceChat": "enabled"
}
```

현재, `voiceChat` 가 일반적으로 지원되는 유일한 기능 플래그입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
