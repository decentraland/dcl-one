# 사용자 신원

그 `UserIdentity` 모듈은 허용합니다 [장면이](/docs/contributor/contributor-ko/content/entity-types/scenes/) 에 접근할 수 있도록 [프로필](/docs/contributor/contributor-ko/content/entity-types/profiles/) 플레이어의 사용자에 대한.

```ts
const UserIdentity = require("~system/UserIdentity");
```

다음 메서드와 타입을 포함합니다:

* [`함수 getUserData`](#getuserdata)
* [`인터페이스 UserData`](#userdata)

### 메서드

이 모듈의 두 메서드는 플레이어의 사용자로부터 일부 정보를 가져옵니다.

#### `getUserData` <a href="#getuserdata" id="getuserdata"></a>

반환합니다 [`UserData`](#userdata) 플레이어의 사용자에 대한.

```ts
interface Request {}

interface Response {
  // 사용자의 프로필 정보, 사용 가능한 경우.
  data?: UserData;
}

function getUserData(Request): Promise<Response>;
```

### 타입

이 모듈에는 타입이 하나만 있습니다: `UserData`.

#### `UserData` <a href="#userdata" id="userdata"></a>

사용자, 그들의 신원 및 아바타에 대한 (부분적일 수도 있는) 정보를 보유합니다.

World Explorers는 이를 다음을 통해 얻을 수 있습니다 [콘텐츠 시스템](/docs/contributor/contributor-ko/content/overview/).

그 [프로필 엔터티 정의](/docs/contributor/contributor-ko/content/entity-types/profiles/) 이 모든 필드와 그 이상을 자세히 설명합니다. 구조와 키는 `UserData` 약간 다르지만, 각 필드의 의미는 동일합니다.

```ts
export interface UserData {
  // 사용자의 Ethereum 주소.
  userId: string;

  // UI에서 그들을 부를 이름.
  displayName: string;

  // 그들이 서명하는 Ethereum 공개 키.
  publicKey?: string;

  // 웹3 기능이 활성화되어 있는지 여부.
  hasConnectedWeb3: boolean;

  // 이 정보의 순차 버전으로, 사용자 업데이트마다 증가합니다.
  version: number;

  // 사용자의 아바타에 대한 정보, 사용 가능한 경우.
  avatar?: {
    // 이 아바타를 렌더링하는 데 필요한 자산에 대한 포인터.
    bodyShape: string;
    wearables: string[];

    // 서로 다른 신체 부위를 위한 16진수 인코딩된 RGB/RGBA 색상(#aabbcc 또는 #aabbccdd).
    skinColor: string;
    hairColor: string;
    eyeColor: string;

    // 이 아바타의 "photos"에 대한 파일 식별자.
    snapshots?: {
      face256: string;
      body: string;
    };
  };
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/runtime-modules/user-identity.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
