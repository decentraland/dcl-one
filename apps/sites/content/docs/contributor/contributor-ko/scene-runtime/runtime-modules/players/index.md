# 플레이어

를 보내 응답해야 합니다. `플레이어` 모듈은 [장면](/docs/contributor/contributor-ko/content/entity-types/scenes/) 에서 플레이어를 찾고 그들의 신원 및 [프로필](/docs/contributor/contributor-ko/content/entity-types/profiles/).

```ts
const Players = require("~system/Players");
```

다음 메서드와 타입을 포함합니다:

* [`function getPlayersInScene`](#getplayersinscene)
* [`function getConnectedPlayers`](#getplayersinscene)
* [`function getPlayerData`](#getplayerdata)
* [`interface Player`](#player)
* [`interface UserData`](#userdata)

### 메서드

이 모듈에는 장면에서 플레이어를 찾는 메서드가 두 개 있고, 프로필을 가져오는 메서드가 하나 있습니다.

#### `getPlayersInScene` <a href="#getplayersinscene" id="getplayersinscene"></a>

다음을 반환합니다 [`Player`](#player) 배열이며, 각 항목에는 `userId` 가 포함되어 있으며 [getPlayerData](#getplayerdata).

```ts
interface Request {}

interface Response {
  players: Player[];
}

function getPlayersInScene(Request): Promise<Response>;
```

#### `getConnectedPlayers` <a href="#getconnectedplayers" id="getconnectedplayers"></a>

```ts
interface Request {}

interface Response {
  players: Player[];
}

function getConnectedPlayers(Request): Promise<Response>;
```

#### `getPlayerData` <a href="#getplayerdata" id="getplayerdata"></a>

다음을 반환합니다 [`UserData`](#userdata) 사용자에 대한 정보가 알려져 있다면, 해당 사용자의 `userId`.

```ts
interface Request {
  // 사용자 식별자(즉, Ethereum 주소).
  userId: string;
}

interface Response {
  // 이 사용자의 프로필 정보(있는 경우).
  data?: UserData;
}

function getPlayerData(Request): Promise<Response>;
```

### 타입

이 모듈에는 타입이 두 개뿐입니다: [`Player`](#player) 및 [`UserData`](#userdata).

#### `Player` <a href="#player" id="player"></a>

다음에 대한 참조를 보유합니다 `userId`.

```ts
interface Player {
  // 사용자 식별자(즉, Ethereum 주소).
  userId: string;
}
```

#### `UserData` <a href="#userdata" id="userdata"></a>

사용자, 그들의 신원 및 아바타에 대한 (부분적일 수 있는) 정보를 보유합니다.

World Explorers는 이 정보를 다음을 통해 얻습니다. [콘텐츠 시스템](/docs/contributor/contributor-ko/content/overview/).

를 보내 응답해야 합니다. [프로필 엔터티 정의](/docs/contributor/contributor-ko/content/entity-types/profiles/) 이 모든 필드와 그 이상을 자세히 설명합니다. 그리고 `UserData` 의 구조와 키는 약간 다르지만, 각 필드의 의미는 같습니다.

```ts
export interface UserData {
  // 사용자 식별자(즉, Ethereum 주소).
  userId: string;

  // UI에서 부를 이름.
  displayName: string;

  // 그들이 서명하는 Ethereum 공개 키.
  publicKey?: string;

  // web3 기능이 활성화되어 있는지 여부.
  hasConnectedWeb3: boolean;

  // 이 정보의 순차 버전으로, 사용자 업데이트 때마다 증가합니다.
  version: number;

  // 가능한 경우 아바타에 대한 정보.
  avatar?: {
    // 이 아바타를 렌더링하는 데 필요한 에셋에 대한 포인터.
    bodyShape: string;
    wearables: string[];

    // 신체 부위별로 Hex로 인코딩된 RGB/RGBA 색상 틴트.
    skinColor: string;
    hairColor: string;
    eyeColor: string;

    // 이 아바타의 "사진"에 대한 파일 식별자.
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
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/runtime-modules/players.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
