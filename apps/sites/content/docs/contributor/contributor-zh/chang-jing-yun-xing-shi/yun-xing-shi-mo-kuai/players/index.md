# 玩家

该 `玩家` 模块允许 [场景](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/) 在其环境中查找玩家并获取有关其身份和 [资料](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/).

```ts
const Players = require("~system/Players");
```

它包含以下方法和类型：

* [`function getPlayersInScene`](#getplayersinscene)
* [`function getConnectedPlayers`](#getplayersinscene)
* [`function getPlayerData`](#getplayerdata)
* [`接口 Player`](#player)
* [`interface UserData`](#userdata)

### 方法

这个模块中有两种方法可用于发现场景中的玩家，另有一种方法可用于获取他们的资料。

#### `getPlayersInScene` <a href="#getplayersinscene" id="getplayersinscene"></a>

返回一个 [`Player`](#player) 数组，每个元素都有一个 `userId` 可用于 [getPlayerData](#getplayerdata).

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

返回 [`UserData`](#userdata) 对于某个用户，如果已知给定的 `userId`.

```ts
接口 Request {
  // 用户标识符（对于非访客，即他们的 Ethereum 地址）。
  userId: string;
}

interface Response {
  // 该用户的资料信息（如有）。
  data?: UserData;
}

function getPlayerData(Request): Promise<Response>;
```

### 类型

这个模块中只有两种类型： [`Player`](#player) 和 [`UserData`](#userdata).

#### `Player` <a href="#player" id="player"></a>

保存对一个 `userId`.

```ts
interface Player {
  // 用户标识符（对于非访客，即他们的 Ethereum 地址）。
  userId: string;
}
```

#### `UserData` <a href="#userdata" id="userdata"></a>

保存有关用户、其身份和头像的信息（可能不完整）。

World Explorers 通过 [内容系统](/docs/contributor/contributor-zh/nei-rong/overview/).

该 [资料实体定义](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/) 详细说明了这些字段及更多内容。虽然 `UserData` 的结构和键略有不同，但每个字段的含义相同。

```ts
export interface UserData {
  // 用户标识符（即他们的 Ethereum 地址）。
  userId: string;

  // 在界面中称呼他们的名称。
  displayName: string;

  // 他们用于签名的以太坊公钥。
  publicKey?: string;

  // 他们是否启用了 web3 功能。
  hasConnectedWeb3: boolean;

  // 这组信息的顺序版本，每次用户更新时递增。
  version: number;

  // 有关其头像的信息（如果可用）。
  avatar?: {
    // 渲染此头像所需资源的指针。
    bodyShape: string;
    wearables: string[];

    // 不同身体部位的十六进制编码 RGB/RGBA 颜色色调。
    skinColor: string;
    hairColor: string;
    eyeColor: string;

    // 此头像“照片”的文件标识符。
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
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/players.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
