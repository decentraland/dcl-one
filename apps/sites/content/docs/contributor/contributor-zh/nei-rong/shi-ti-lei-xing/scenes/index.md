# 场景

场景是 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 在 Decentraland 的不同部分产生行为的实体。它们可以跨越多个土地地块，玩家在 World Explorer 中穿过它们时会执行场景提供的特殊代码。

你可以在 [该场景的 manifest](https://peer.decentraland.org/content/contents/bafkreidihplrc5cxarjkji2enmzhicpjoa2vrlqyyffnholmxb3o2xft3u) 在基金会的内容服务器上找到完整的真实示例。

### 指针 <a href="#pointers" id="pointers"></a>

场景有一个或多个关联的 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) ，其形式为地块坐标字符串，例如 `"0,0"`。运行该场景的每个地块都将列在实体 manifest 的 [`指针` 数组](/docs/contributor/contributor-zh/nei-rong/entities/#properties) 中，所有这些指针都会由内容服务器解析到该场景。

换句话说，你可以选择某个地块的 x/y 坐标，并通过查询内容服务器的 `"<x>,<y>"` 指针来定位它所运行的场景。这是 World Explorers 判断当玩家在世界中导航时是否启用某个场景行为的方式。

典型的 `指针` 数组看起来像这样：

```json
{
  "pointers": [
    "-113,-134",
    "-113,-133",
    "-113,-132",
  ],
  // ... 其他实体属性
}
```

这些数据也可在 `metadata.scene.parcels` 字段中找到，详见下文，以防你只想在 manifest 中保存场景特定对象。

### 元数据字段

对于 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 类型为 `场景` 的 `metadata` 顶层字段，有几个特殊属性。

| 字段                    | 值                                                                                        |
| --------------------- | ---------------------------------------------------------------------------------------- |
| `owner`               | 关于场景维护者的信息。                                                                              |
| `main`                | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 指向该场景的主 JavaScript 文件。 |
| `tags`                | 描述此场景的一组字符串标签。                                                                           |
| `display`             | 供 Explorers 向玩家展示的场景信息（见下文）。                                                             |
| `场景`                  | 该场景激活的地块及其中心位置（见下文）。                                                                     |
| `contact`             | 用于联系场景创建者或维护者的姓名和电子邮件地址（见下文）。                                                            |
| `spawnPoints`         | 玩家进入该场景时的位置和摄像机角度（见下文）。                                                                  |
| `requiredPermissions` | 建议 Explorer 向玩家请求的权限（见下文）。                                                               |
| `featureToggles`      | 功能标志的设置                                                                                  |


> [!NOTE]
> 请记住，实体及其元数据可以拥有自定义字段。你可能会发现一些这里未列出的属性。


#### 显示

在 `metadata.display`中，你会找到一些有助于在玩家进入场景前向其介绍场景的属性。

| 字段                | 值                                                                                             |
| ----------------- | --------------------------------------------------------------------------------------------- |
| `title`           | 此场景的显示名称。                                                                                     |
| `description`     | 此场景的扩展描述。                                                                                     |
| `favicon`         | *已弃用* 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 指向一个图标，在该场景处于活动状态时显示。 |
| `navmapThumbnail` | 该 [内部文件名](/docs/contributor/contributor-zh/nei-rong/entities/#files) 指向该场景在世界地图中的缩略图。            |

示例：

```json
{
  "title": "my_cool_scene",
  "favicon": "favicon.png",
  "navmapThumbnail": "thumbnail.png"
}
```

#### 场景

该 `metadata.scene` 属性是一个描述该场景在世界地图中位置的对象。

| 字段        | 值             |
| --------- | ------------- |
| `parcels` | 运行该场景的地块指针数组。 |
| `base`    | 该场景原点的地块指针。   |

实际上，它看起来像这样：

```json
{
  "parcels": [
    "17,-112", 
    "17,-113"
  ],
  "base": "17,-112"
}
```

该 `base` 字段始终包含在 `parcels` 数组中。

#### 联系

该 `metadata.contact` 对象包含场景所有者或维护者的联系信息。

| 字段      | 值                |
| ------- | ---------------- |
| `name`  | 此联系人的姓名。         |
| `email` | （可选）用于联系的电子邮件地址。 |

JSON 中：

```json
{
  "name": "cool_scene_maker",
  "email": "hello@decentraland.org"
}
```

虽然 `name` 字段会在每个场景中填入，但在实际使用中你会发现 `email` 字段通常是空字符串。这是场景创建者出于隐私考虑所做的选择。

#### 出生点

该 `metadata.spawnPoints` 字段定义了一个点和摄像机方向的数组，用于决定玩家进入场景时应出现的位置，以及他们最初应朝向哪里。必须至少定义一个出生点。

数组中的每个项都有一些属性：

| 字段             | 值                                  |
| -------------- | ---------------------------------- |
| `name`         | 用于标识此出生点的标签。                       |
| `position`     | 在一个 `{ x, y, z }` 对象中的起始位置，使用浮点坐标。 |
| `cameraTarget` | 在一个 `{ x, y, z }` 对象中的起始位置，使用浮点坐标。 |
| `默认`           | 是否使用此出生点，除非另有指定。                   |

例如：

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
> **📔 注**：出生区域和摄像机目标可以使用 Creator Hub 设置。有关更多信息，请访问 [出生区域](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/spawn-areas.md) 文档。


#### 所需权限 <a href="#permissions" id="permissions"></a>

在 `metadata.requiredPermissions` 你会找到一个众所周知的字符串数组，用于声明在 World Explorer 允许某些操作之前应向玩家请求哪些权限。这取决于 Explorer，其他为不同目的构建的客户端可能会选择忽略它。

这是当前支持的权限集：

| 权限                                  | 目的                                 |
| ----------------------------------- | ---------------------------------- |
| `USE_FETCH`                         | 允许场景执行外部 HTTP 请求。                  |
| `USE_WEBSOCKET`                     | 允许场景使用 Websocket API 建立外部连接。       |
| `OPEN_EXTERNAL_LINK`                | 允许场景打开一个 URL（在浏览器标签页或 web view 中）。 |
| `USE_WEB3_API`                      | 允许场景与钱包通信。                         |
| `ALLOW_TO_TRIGGER_AVATAR_EMOTE`     | 允许场景用表情动作为玩家的头像制作动画。               |
| `ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE` | 允许场景改变玩家的位置。                       |

#### 功能开关

该 `metadata.featureToggles` 字段允许场景指示 World Explorer 的某些功能应启用还是禁用。

该字段包含一个如下形式的对象 `{ [featureName]: 'enabled' | 'disabled' }`，例如这个：

```json
{
  "voiceChat": "enabled"
}
```

目前， `voiceChat` 是唯一常见支持的功能标志。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
