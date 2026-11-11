# 服装

Outfits 是玩家保存的装扮。一个 outfit 由身体形状、皮肤、头发、眼睛和可穿戴物配置组成。

### 指针 <a href="#pointers" id="pointers"></a>

服装搭配 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 是所有者的以太坊地址，带有一个 "`:outfits`" 后缀。例如：

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df:outfits
```

### 元数据字段

| 字段                   | 值                                         |
| -------------------- | ----------------------------------------- |
| `装扮`                 | 每个装扮的描述数组（见下文）。                           |
| `namesForExtraSlots` | 用于验证额外槽位的已拥有名称数组。对于每个已拥有名称，将额外授予最多 5 个槽位。 |

### 服装搭配

metadata.outfits\[] 中的每个装扮 `metadata.outfits[]` 代表用户的一个装扮，并具有若干属性：

| 字段       | 值            |
| -------- | ------------ |
| `slot`   | 已保存装扮的槽位编号。  |
| `outfit` | 已保存的装扮（见下文）。 |

示例：

```json
{
  "slot": 1,
  "outfit": {
    // 见下文
  }
}
```

#### 装扮字段

该 `metadata.outfits[].outfit` 字段包含保存玩家装扮并将其渲染所需的全部信息。

| 字段          | 值                                                                                               |
| ----------- | ----------------------------------------------------------------------------------------------- |
| `bodyShape` | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 对应头像的身体形状实体。                           |
| `eyes`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像眼睛的对象。                                                            |
| `hair`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像头发的对象。                                                            |
| `skin`      | 一个带有 `颜色` 形式为 `{ r, g, b }` 头像皮肤的对象。                                                            |
| `可穿戴物品`     | 一个 [可穿戴物品指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/#pointers) 被头像使用。 |

示例如下：

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
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/outfits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
