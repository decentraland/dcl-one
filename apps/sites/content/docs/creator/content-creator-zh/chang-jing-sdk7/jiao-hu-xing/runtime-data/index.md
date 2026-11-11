> 从场景运行所在的上下文以及场景本身获取数据。

# 运行时数据

## 获取 Decentraland 时间

Decentraland 遵循一个昼夜循环，完成一次需要 2 小时，因此每天有 12 个完整循环。玩家还可以更改设置来体验某个固定时段，例如始终以晚上 10 点的夜空来查看 Decentraland。因此，Decentraland 中的时间可能因玩家而异。

使用 `getWorldTime()` 用于获取玩家在 Decentraland 中所体验到的当天时间。

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```


> [!NOTE]
> **💡 提示**： `getWorldTime()` 该函数是异步的。见 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/) 如果你对此不熟悉。


`getWorldTime()` 返回一个包含一个 `seconds` 属性。该属性表示自一天开始以来已经过去了多少秒（按 Decentraland 时间计算），假设完整循环持续 24 小时。将秒数除以 60 可得分钟，再除以 60 可得自一天开始以来的小时数。例如，如果 `seconds` 值是 *36000*，那就对应 *上午 10 点*.

在 Decentraland 时间中，太阳总是在 6:15 升起，在 19:50 落下。

你可以利用这些信息来相应地调整场景，例如在天亮时播放鸟鸣声、在天黑时播放蟋蟀声，或者在天黑时打开路灯的自发光材质。

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
  if (time.seconds < 6.25 * 60 * 60 || time.seconds > 19.85 * 60 * 60) {
    // 夜间
    console.log('播放蟋蟀声')
  } else {
    // 白天
    console.log('播放鸟鸣声')
  }
})
```

## 获取 realm 数据

Decentraland 中的玩家存在于多个彼此分离的 *realms*. 不同 realm 中的玩家彼此看不到、无法交互或聊天，即使他们站在同一块地块上也是如此。这样将玩家分开管理，使 Decentraland 能够处理无限数量的玩家而不会遇到任何限制。它还会把位于相近区域的玩家配对，以确保交互玩家之间的 ping 时间可接受。

如果你的场景会向一个 [第三方服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/third-party-servers/) 发送数据，以便在玩家之间实时同步更改，那么通常很重要的一点是，更改只应在同一 realm 中的玩家之间同步。你应该将属于某个 realm 的所有更改与另一个 realm 的更改分开处理。否则，玩家会看到一些诡异的变化，却没人真正做出这些更改。

```ts
import { getRealm } from '~system/Runtime'

executeTask(async () => {
  const { realmInfo } = await getRealm({})
  if (!realmInfo) return
  console.log(`You are in the realm: `, realmInfo.realmName)
})
```


> [!NOTE]
> **💡 提示**： `getRealm()` 该函数是异步的。见 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/) 如果你对此不熟悉。


Decentraland 通过一个去中心化的通信服务器网络来处理玩家之间的通信（包括玩家位置、聊天、messageBus 消息以及智能物品状态变更），这些服务器中的每一个都称为一个 **Realm**。这些服务器中的每一个都可以支持多个彼此独立的 **房间** （也称为 **islands**），每个房间将 Decentraland 地图上彼此靠近的一组不同玩家分组。

该 `getRealm()` 函数返回以下信息：

* `baseUrl`: *（字符串）* realm 服务器的域名
* `realmName`: *（字符串）* realm 服务器的名称
* `networkId`: *（数字）* Ethereum 网络
* `commsAdapter`: *（字符串）* Comms 适配器，移除所有查询参数（凭据）
* `isPreview`: *（布尔值）* 如果场景作为本地预览运行，而不是在 Decentraland 中发布，则为 True。
* `isConnectedSceneRoom`: *（布尔值）* 如果用户已连接到场景房间，则为 True。


> [!WARNING]
> **📔 注意**： `layer` 该属性已弃用，应避免使用。


随着玩家在地图中移动，他们可能会切换房间，与现在离他们最近的那些玩家分到一起。房间边界也会动态调整，以容纳一个可管理的人数组，因此即使玩家站着不动，随着玩家进入和离开世界，玩家也可能发现自己身处另一个房间。同一个 `room` 中的玩家彼此通信，并且即使他们相距太远而看不到对方，也会通过 MessageBus 共享消息。同一服务器但位于不同房间的玩家目前不会通信，但随着他们在地图中移动并切换房间，之后可能会开始通信。

要响应与玩家的 realm 或房间相关的变化，请使用 `onChange` 位于 `RealmInfo` 组件，引擎会将其添加到 `engine.RootEntity`。该组件保存着与 `getRealm()`.

```ts
import { engine, RealmInfo } from '@dcl/sdk/ecs'

export function main() {
	RealmInfo.onChange(engine.RootEntity, (realmInfo) => {
		if (!realmInfo) return
		console.log('Realm changed: ', realmInfo.realmName)
	})
}
```


> [!WARNING]
> **📔 注意**：当场景首次加载时，玩家可能还没有分配到房间。Explorer 最终会为玩家分配一个房间，但这有时会在场景加载后几秒才发生。


## 获取玩家平台

玩家可以通过多种平台访问 Decentraland，包括官方桌面应用、已弃用的网页和桌面版本，以及 [其他实验性客户端](https://github.com/decentraland/protocol-squad) ，这些客户端是为其他引擎构建的。

使用 `getExplorerInformation()` 来了解当前玩家正在通过哪个平台运行 Decentraland。

```ts
import { getExplorerInformation } from '~system/Runtime';

executeTask(async () => {
  let data = await getExplorerInformation({})
  console.log("PLATFORM: ", data.platform, " AGENT: ", data.agent)
})
```

使用官方 Decentraland 桌面应用时，该函数应返回以下数据：

```
{
    agent: unity-explorer,
    platform: desktop
}
```

使用官方 Decentraland 桌面应用的玩家，其体验很可能比浏览器中的玩家流畅得多，因为浏览器会对标签页可使用的机器处理能力施加性能限制。它们也会缺少许多功能，比如摄像机控制、动态灯光、冻结玩家移动、UI 增强等。

## 获取玩家语言

使用 `getPlayerLanguage()` 来了解玩家客户端设置成了哪种语言。它返回一个 [BCP 47](https://en.wikipedia.org/wiki/IETF_language_tag) 标签，例如 `'en'`, `'es'` 或 `'pt-BR'`。可用它来选择显示场景文本的哪种翻译。

```ts
import { getPlayerLanguage, onPlayerLanguageChanged } from '@dcl/sdk/platform'

console.log('LANGUAGE: ', getPlayerLanguage())

// 响应玩家在会话中途切换语言
onPlayerLanguageChanged.add(({ language }) => {
  console.log('LANGUAGE CHANGED TO: ', language)
})
```

`onPlayerLanguageChanged` 在玩家于你的场景中更改客户端语言时触发。旧版客户端从不触发它。


> [!WARNING]
> **📔 注意**: `getPlayerLanguage()` 返回 `'en'` ，直到 Explorer 将其语言报告回场景为止，这通常发生在场景启动后不久。在 `'en'` 不报告语言的客户端上也会返回 `@dcl/sdk` 7.30.0 或更新版本。


## EngineInfo 组件

该 `EngineInfo`组件会跟踪场景生命周期相关的数据，这有时有助于判断某个事件相对于场景初始化是在何时发生的。

该组件会被添加到 `engine.RootEntity`.

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo) return

  console.log(
    '--------------' +
      '\nframeNumber: ' +
      engineInfo.frameNumber +
      '\ntickNumber: ' +
      engineInfo.tickNumber +
      '\ntotalRuntime: ' +
      engineInfo.totalRuntime +
      '\nsceneHidden: ' +
      engineInfo.sceneHidden +
      '\n--------------'
  )
})
```

你可以使用 `sceneHidden` 用于在场景不可见时暂停资源密集型工作：

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (engineInfo?.sceneHidden) return

  // 正常的游戏逻辑只会在场景可见时运行
})
```

该 `EngineInfo` 组件保存以下数据：

* `frameNumber`：引擎的帧计数器。
* `totalRuntime`：该场景的总运行时间，单位为秒。
* `tickNumber`：按 [ADR-148](https://adr.decentraland.org/adr/ADR-148).
* `sceneHidden`: `true` ：场景在 Explorer 的全屏 UI 后面被隐藏时。这里包括加载界面，以及地图、背包和设置菜单。可用它在玩家看不到场景时静音音频并暂停高开销工作。


> [!WARNING]
> **📔 注意**： `EngineInfo` 组件必须通过以下方式导入
> 
> > `import { EngineInfo } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。


### 响应加载界面淡出

该 `scene_hidden` 字段会告诉你玩家是否真的能看到你的场景，还是被 Explorer 的全屏 UI 遮住了。加载界面显示时， `sceneHidden` 是 `true`。当加载界面淡出、玩家第一次看到世界时，它会变为 `false`.

它会变为 `true` 之后，每当 Explorer 的全屏 UI 再次遮住场景时，例如地图、背包或设置菜单，它也会再次变为相同状态。这使它成为一个很好的信号，用于静音 [音频](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/) 并暂停高开销系统，然后在它恢复为 `false`.

这是场景得知首次露出何时发生的唯一方式。可用它来挡住任何本来会在加载界面后面播放、而玩家会错过的内容：开场过场动画、欢迎音效、只有在被观看时才看起来合理的 tween、开场 UI，或者只有在玩家真正到达后才应计数的分析事件。

```ts
import { engine, EngineInfo } from '@dcl/sdk/ecs'

function onSceneRevealed() {
  // 现在玩家正在看场景，在这里开始介绍
  console.log('加载界面刚刚淡出了')
}

engine.addSystem(function waitForSceneRevealed() {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo || engineInfo.sceneHidden) return

  // 只运行一次
  engine.removeSystem(waitForSceneRevealed)
  onSceneRevealed()
})
```


> [!WARNING]
> **📔 注意**：你的场景在 `sceneHidden` 是 `true`时会正常运行，只是不会被显示。不要用这个字段来暂停场景逻辑，而是用它来判断玩家本应何时看到内容。
> 
> `scene_hidden` 需要最新版本的 `@dcl/sdk` 以及较新的 Decentraland Explorer 版本。在旧版客户端上，该字段会保持其默认值 `false`，因此等待它的场景仍会运行——只是不会与加载界面的淡出同步。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/runtime-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
