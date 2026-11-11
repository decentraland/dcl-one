# 个人资料

资料是玩家的基本描述，包含其游戏内名称和化身等信息。

它们在内容服务器中作为常规内容提供 [实体](/docs/contributor/contributor-zh/nei-rong/entities/)，不过 World Explorers 通常会利用通信系统即时获取最新版本。

系统已准备好支持同一所有者拥有多个身份，全部作为实体清单中的化身包含在内。不过在实践中，绝大多数玩家只有一个。

### 指针 <a href="#pointers" id="pointers"></a>

资料 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 是所有者的 Ethereum 地址，不带任何前缀或后缀。例如：

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df
```

### 元数据字段

| 字段   | 值                       |
| ---- | ----------------------- |
| `化身` | 一个数组，包含所有者每个化身的描述（见下文）。 |

### 化身

每个 `metadata.avatars[]` 都代表同一所有者的一个身份，并具有若干属性，使客户端能够显示资料、在世界中渲染化身并联系所有者。

| 字段               | 值                                                                         |
| ---------------- | ------------------------------------------------------------------------- |
| `userId`         | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 解析到（或曾用于解析到）此资料。 |
| `name`           | 该玩家的显示名称。                                                                 |
| `email`          | （可选）该玩家的电子邮件地址。                                                           |
| `description`    | （可选）由玩家选择用于描述自己的文本。                                                       |
| `ethAddress`     | 该玩家的 Ethereum 地址（目前等同于其 `userId`).                                        |
| `hasClaimedName` | 是否 `name` 字段是一个已认领的 ENS 名称。                                               |
| `unclaimedName`  | 没有 ENS 名称的用户（例如访客）使用的临时名称。                                                |
| `tutorialStep`   | 该玩家的教程进度。                                                                 |
| `化身`             | 玩家在世界中的化身属性（见下文）。                                                         |

示例：

```json
{
  "userId": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "name": "Bob",
  "email": "",
  "hasClaimedName": false,
  "description": "",
  "ethAddress": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "avatar": {
    // 见下文
  }
}
```

#### 化身字段

该 `metadata.avatars[].avatar` 字段包含 World Explorer（或其他客户端，例如独立化身编辑器）渲染玩家所需的全部信息。

| 字段          | 值                                                                                               |
| ----------- | ----------------------------------------------------------------------------------------------- |
| `bodyShape` | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 对应头像的身体形状实体。                           |
| `快照`        | 一个包含此化身图像的对象（见下文）。                                                                              |
| `eyes`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像眼睛的对象。                                                            |
| `hair`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像头发的对象。                                                            |
| `skin`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像皮肤的对象。                                                            |
| `可穿戴物品`     | 一个 [可穿戴物品指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/#pointers) 被头像使用。 |

示例如下：

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
  "snapshots": {
    // 见下文
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

#### 快照

该 `metadata.avatars[].avatar.snapshots` 字段包含 [图像的文件标识符](/docs/contributor/contributor-zh/nei-rong/file-system/#identifiers) （包含在 [`内容` 顶层字段](/docs/contributor/contributor-zh/nei-rong/entities/#properties)中）。每个属性都是一种图像类型，目前有两种： `face256` （缩略图）和 `body` （全尺寸版本）。

例如：

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
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
