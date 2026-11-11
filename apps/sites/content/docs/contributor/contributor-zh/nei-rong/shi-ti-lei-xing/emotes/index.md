# 表情动作

表情动作是 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 用于存放玩家头像动画的文件。

它们包含适用于不同身体形状的 GLB 格式文件。

### 指针 <a href="#pointers" id="pointers"></a>

每个表情动作恰好关联一个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/)，它指明它们所属的集合以及在集合中的索引。其 URN 形式如下：

```
urn:decentraland:matic:collections-v2:<collection address>:<item index>
```

例如， `指针` 一个表情动作的数组如下：

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... 其他实体属性
}
```

### 元数据字段

表情动作的大部分基础字段与 [可穿戴物品](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/). 该 `emoteDataADR74` 属性包含表情动作特有的信息。

| 字段                  | 值                                                                                      |
| ------------------- | -------------------------------------------------------------------------------------- |
| `ID`                | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 指向（或曾经指向）此表情动作的。              |
| `name`              | 此表情动作在 [collection](/docs/contributor/contributor-zh/nei-rong/collections/).              |
| `description`       | 此表情动作的扩展描述。                                                                            |
| `图片`                | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 包含此表情动作图片。           |
| `缩略图`               | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 用于 256x256 版本的 `图片`. |
| `稀有度`               | 以下之一 `普通`, `罕见`, `稀有`, `史诗`, `传奇`, `异域`, `神话` 或 `唯一`.                                  |
| `i18n`              | 用于 `name` 字段的翻译数组。                                                                     |
| `collectionAddress` | 包含此表情动作的集合的以太坊地址。                                                                      |
| `指标`                | 关于动画的一些有用度量（见下文）。                                                                      |
| `emoteDataADR74`    | 此表情动作的扩展元数据，按 [ADR-74](https://adr.decentraland.org/adr/ADR-74) （见下文）。                 |

典型的 JSON 如下：

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0",
  "name": "Funny Dance",
  "description": "Move around like a champ",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "rare",
  "i18n": [
    { "code": "es", "text": "Danza Graciosa" }
  ],
  "collectionAddress": "0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5",
  "metrics": {
    // 度量对象（见下文）。
  },
  "emoteDataADR74": {
    // ADR-74 中定义的扩展元数据（见下文）。
  }
}
```

### 指标

在 `metadata.metrics` 对象中，你会找到与此表情动作一起打包的动画的一些简单度量。示例：

```json
{
  "triangles": 0,
  "materials": 0,
  "textures": 0,
  "meshes": 0,
  "bodies": 0,
  "entities": 1
}
```

### 表情动作数据 ADR-74

此对象包含 World Explorers（或其他图形应用程序）使用此表情动作为模型制作动画所需的字段。

| 字段     | 值                                                     |
| ------ | ----------------------------------------------------- |
| `类别`   | 以下之一 `舞蹈`, `特技`, `问候`, `趣味`, `姿势`, `反应`, `恐怖` 或 `其他`. |
| `表现形式` | 与不同身体形状关联的动画文件数组。                                     |
| `tags` | 用于描述此表情动作的字符串标签数组。                                    |
| `循环`   | `true` 动画结束后是否应重复播放自身。                                |

为清楚起见，以下是一些 JSON：

```json
{
  "category": "dance",
  "representations": [
    // 不同身体形状的动画信息（见下文）。
  ],
  "tags": [ "carnaval", "dance" ],
  "loop": true
}
```

#### 表现形式

中的每一项 `metadata.emoteDataADR74.representations` 字段都定义了适用于每种身体形状的动画文件。

| 字段     | 值                                                                             |
| ------ | ----------------------------------------------------------------------------- |
| `身体形状` | 适用的身体形状数组 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/).             |
| `主文件`  | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 用于主 GLB 文件。 |
| `内容`   | 所有文件，包括 `主文件` 以及它可以引用的任何其他文件。                                                 |

例如， `表现形式` 项：

```json
{
  "bodyShapes": [ "urn:decentraland:off-chain:base-avatars:BaseMale" ],
  "mainFile": "male/funnydance.glb",
  "contents": [
    "male/funnydance.glb"
  ]
}
```

通常你会找到两个表现形式条目，一个对应 `BaseMale` 以及一个对应 `BaseFemale`.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
