# 可穿戴物

可穿戴物是 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 其中包含供玩家穿戴并自定义其头像的物品。

该 `可穿戴物品` 玩家的 [资料](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/profiles/) 包含一个指向他们当前正在穿戴物品的指针列表。

每件可穿戴物都属于一个 *类别*，表示它在头像身体上所占的位置，并且可以隐藏或替换其他类别（例如，一条长裙可以作为上身服装穿戴，但会完全遮住腿部）。

共有 18 个可穿戴类别，在各种元数据字段中被引用： `body_shape`, `hair`, `eyes`, `眉毛`, `胡须`, `嘴巴`, `上半身`, `下半身`, `脚部`, `帽子`, `头盔`, `面具`, `头冠`, `头顶`, `耳环`, `眼镜`, `skin` 和 `手部穿戴`.

可穿戴实体包含适用于不同身体形状的 GLB 格式文件。

### 指针 <a href="#pointers" id="pointers"></a>

可穿戴物恰好只有一个关联的 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/)，它指明了 [collection](/docs/contributor/contributor-zh/nei-rong/collections/) 它所属的集合及其在其中的标识符。

当可穿戴物是链上资产时，该指针是如下形式的 URN：

```
urn:decentraland:<blockchain>:collections-v2:<collection address>:<item index>
```

例如， `指针` 链上可穿戴物的数组看起来如下：

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... 其他实体属性
}
```

当可穿戴物是链下资产时，例如新头像的默认可穿戴物，它将采用如下形式：

```
urn:decentraland:off-chain:<collection>:<item name>
```

这里的集合是一个名称，而不是地址，物品标识符也是名称而不是索引。

该 `指针` 链下可穿戴物的数组看起来如下：

```json
{
  "pointers": [
    "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  // ... 其他实体属性
}
```

### 元数据字段

可穿戴物与 [表情动作](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes/)共享大部分基础字段。 `数据` 属性是存放可穿戴物特定信息的位置。

| 字段                  | 值                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------- |
| `ID`                | 该 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 解析到（或曾经解析到）该可穿戴物的。           |
| `name`              | 该可穿戴物在 [collection](/docs/contributor/contributor-zh/nei-rong/collections/).             |
| `description`       | 对该可穿戴物的扩展描述。                                                                          |
| `图像`                | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 的图片。                |
| `缩略图`               | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 的 256x256 版本。 `图像`. |
| `稀有度`               | 以下之一 `普通`, `不常见`, `稀有`, `史诗`, `传说`, `异域`, `神话` 或 `独特`.                                |
| `i18n`              | 针对 `name` 字段的一组翻译。                                                                    |
| `collectionAddress` | 包含此可穿戴物的集合的以太坊地址。                                                                     |
| `指标`                | 关于动画的一些有用测量数据（见下文）。                                                                   |
| `数据`                | 此可穿戴物的扩展元数据（见下文）。                                                                     |

在典型的 JSON 形式中：

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632b4e7226816a16139fb33:0",
  "name": "漂亮连衣裙",
  "description": "穿起来非常好看",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "不常见",
  "i18n": [
    { "code": "es", "text": "漂亮连衣裙" }
  ],
  "collectionAddress": "0x11a6879861f36cbad632b4e7226816a16139fb33",
  "metrics": {
    // 测量对象（见下文）。
  },
  "data": {
    // 扩展元数据（见下文）。
  }
}
```

### 指标

在 `metadata.metrics` 对象中，你会找到该可穿戴物所打包模型的一些简单测量数据。示例：

```json
{
  "entities": 1,
  "bodies": 12,
  "materials": 2,
  "meshes": 12,
  "textures": 2,
  "triangles": 3321
}
```

### 数据

该 `metadata.data` 对象中存放可穿戴物特定属性。

| 字段     | 值                     |
| ------ | --------------------- |
| `类别`   | 上面列出的可穿戴类别之一。         |
| `表示`   | 与不同身体形状相关联的文件数组（见下文）。 |
| `隐藏`   | 此可穿戴物隐藏的类别数组。         |
| `替换`   | 此可穿戴物完全替换的类别数组。       |
| `tags` | 描述此可穿戴物的一组字符串标签。      |

#### 表示

代表数组中的每一项将一个身体形状与一组模型关联起来，并且可以添加有关每个类别显示哪些可穿戴物的规则。

| 字段                 | 值                                                                                                  |
| ------------------ | -------------------------------------------------------------------------------------------------- |
| `bodyShapes`       | 一个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 到身体形状实体。                                 |
| `mainFile`         | 用于开始渲染的 3D 模型文件。                                                                                   |
| `内容`               | 文件名数组（存在于顶层 [`内容` 字段](/docs/contributor/contributor-zh/nei-rong/entities/#properties)中），供 `mainFile`. |
| `overrideHides`    | 替代上面的 `隐藏` 列表，用于此表示。                                                                               |
| `overrideReplaces` | 替代上面的 `替换` 列表，用于此表示。                                                                               |

这是一个典型的 `表示` 条目：

```json
{
  "bodyShapes": [
      "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  "mainFile": "male/190.glb",
  "contents": [
      "male/190.glb"
  ],
  "overrideHides": [ "lower_body" ],
  "overrideReplaces": []
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
