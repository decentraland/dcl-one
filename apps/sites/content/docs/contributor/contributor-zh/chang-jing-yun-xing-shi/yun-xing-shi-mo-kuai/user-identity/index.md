# 用户身份

该 `UserIdentity` 模块允许 [场景](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/) 访问 [资料](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/) 以获取玩家的用户资料。

```ts
const UserIdentity = require("~system/UserIdentity");
```

它包含以下方法和类型：

* [`function getUserData`](#getuserdata)
* [`interface UserData`](#userdata)

### 方法

此模块中的两个方法会从玩家的用户中获取部分信息。

#### `getUserData` <a href="#getuserdata" id="getuserdata"></a>

返回 [`UserData`](#userdata) 以获取玩家的用户资料。

```ts
interface Request {}

interface Response {
  // 用户的资料信息（如果可用）。
  data?: UserData;
}

function getUserData(Request): Promise<Response>;
```

### 类型

此模块中只有一种类型： `UserData`.

#### `UserData` <a href="#userdata" id="userdata"></a>

保存有关用户、其身份和头像的信息（可能不完整）。

World Explorers 可通过以下方式获取它： [内容系统](/docs/contributor/contributor-zh/nei-rong/overview/).

该 [资料实体定义](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/) 详细说明了这些字段及更多内容。虽然 `UserData` 的结构和键略有不同，但每个字段的含义相同。

```ts
export interface UserData {
  // 用户的以太坊地址。
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

    // 用于不同身体部位的十六进制编码 RGB/RGBA 颜色（#aabbcc 或 #aabbccdd）。
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
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/user-identity.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
