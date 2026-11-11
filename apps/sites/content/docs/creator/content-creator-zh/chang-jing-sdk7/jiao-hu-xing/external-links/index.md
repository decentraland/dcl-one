> 链接到其他场景或外部网站

# 外部链接

你可以在场景中添加指向其他内容的链接，可以是其他场景，也可以是外部网站。

## 在 Creator Hub 中使用场景编辑器

添加外部链接或传送的最简单方法是使用场景编辑器。使用 **传送** [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 来添加前往 Genesis City 中另一个场景的传送，或者使用任意 **社交链接** 智能物品来添加指向外部网站的链接。

![](/docs/_remote/71809788940cd245e36af334.webp)

## 传送门

要将玩家传送到另一个场景，请调用以下函数，并指定你希望玩家传送到的坐标。

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ worldCoordinates: { x: -51, y: 1 } })
```

玩家在被传送前会看到一个确认界面，该界面显示来自目标场景的 `scene.json 文件`，包括场景 `名称`, `描述` 和 `navmapThumbnail`。参见 [场景元数据](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/) 有关如何设置这些数据的详细信息。

请注意，传送会将你带到指定坐标所在的场景，但不一定是那些完全相同的坐标。这意味着，当前往一个包含多个地块的场景时，玩家可能不会落在指定的相同坐标上，而是会落在该场景创建者指定的某个出生点。

要将玩家移动到当前场景中的另一组坐标，请使用 `movePlayerTo()` 函数代替。参见 [移动玩家](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/move-player/).

## 传送到 WORLD

要将玩家发送到未发布在公开 Genesis City 地图上的场景，而是发送到一个独立的 [Decentraland WORLD](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)，传入一个 `realm` 设置为 `teleportTo()`.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ realm: 'mannakia.dcl.eth' })
```

`realm` 接受一个世界名称，例如 `mannakia.dcl.eth`，或一个 realm URL。

如果不提供 `worldCoordinates`，玩家将落在目标地点的某个出生点上。添加坐标可在该世界中选择特定地块：

```ts
// 落在某个特定世界的 12,34 地块上
teleportTo({
	realm: 'mannakia.dcl.eth',
	worldCoordinates: { x: 12, y: 34 },
})
```

玩家在切换 realm 前会看到一个确认界面。该界面会询问 *"你确定要进入这个世界吗？"* 并显示目标位置（世界名称或 realm URL 的主机名）。只有在玩家接受后才会被移动。

要将玩家从某个世界送回 Genesis City，请不要包含 `realm` 字段，该字段默认指向 Genesis City。


> [!WARNING]
> **📔 注意**： `realm` 字段需要 `@dcl/sdk` 7.28.0 或更新版本。


### changeRealm 已弃用

较旧的场景使用 `changeRealm()` 来到达某个世界。它仍然可用，但请改用 `teleportTo({ realm })` 代替。 `changeRealm()` 也会显示确认界面；其可选的 `消息` 字段只会替换默认的提示文本。

`changeRealm()` 一旦玩家接受提示就会立即完成，而不是等到新 realm 实际生效时才完成。 `teleportTo()` 紧接着的调用会作用于 **旧的** realm，并把玩家传送到错误的位置。 `teleportTo({ realm })` 会在一次调用中完成这两个步骤，因此不存在这个时间差。

## 外部链接

要添加指向外部网站的链接，请使用 `openExternalUrl()` 命令。

```ts
import { openExternalUrl } from '~system/RestrictedActions'

openExternalUrl({ url: 'google.com' })
```

为防止有人滥用此功能向玩家发送垃圾内容，只能在 `openExternalUrl` 通过实体上的明确点击或按钮事件来调用。不能通过计时器、碰撞区域或全局点击事件来调用此函数。请参阅 [按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/) 以了解如何执行此操作的详细信息。

当 `openExternalUrl` 被调用时，玩家会看到一个确认界面，界面会告知他们链接将把他们带到哪里，并且他们可以选择接受或拒绝访问该链接。

链接会在新标签页中打开，原来的标签页会保留在 Decentraland 中。

如果玩家勾选 *信任此域名* 复选框，只要链接来自同一个场景且指向同一域名，他们在本次会话中就不会再收到提示。

## 复制到剪贴板

要将字符串复制到玩家的剪贴板，请使用 `copyToClipboard()`。之后，当玩家在 *粘贴* 到 Decentraland 聊天或其设备上的任何其他应用中时，粘贴的将是你的字符串。

```ts
import { copyToClipboard } from "~system/RestrictedActions"

copyToClipboard( { text: 'My text to copy' } )
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/external-links.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
