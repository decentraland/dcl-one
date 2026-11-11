> 旧版函数

# 已弃用函数

以下函数均为旧版接口，应避免使用。它们目前仍可工作，但未来可能不再受支持。所有这些示例都包含了通过其他方式获取相同信息或实现相同结果的链接。

## 玩家进入或离开场景


> [!WARNING]
> **📔 注意**： `onEnterSceneObservable` 和 `onLeaveSceneObservable` 事件在 SDK 7.x 中已弃用。请使用 `onEnterScene` 替代，参见 [玩家进入或离开场景](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-enters-or-leaves-scene).


每当一个头像进入或离开构成你场景的地块，或者传送进出时，都会创建一个你可以监听的事件。此事件会被所有头像触发，包括玩家自己的头像。

```ts
import {
	onEnterSceneObservable,
	onLeaveSceneObservable,
} from '@dcl/sdk/observables'

onEnterSceneObservable.add((player) => {
	console.log('player entered scene: ', player.userId)
})

onLeaveSceneObservable.add((player) => {
	console.log('player left scene: ', player.userId)
})
```


> [!WARNING]
> **📔 注意**：该事件只会响应当前在本地渲染的玩家。在场景规模超过可视范围的大型场景中，出现在对角区域的玩家可能不会被注册。如果区域中的玩家数量超过了 Decentraland 服务器中某个 island 的承载能力，那么不在同一个 island 上的玩家将不可见，这些事件也不会跟踪他们。


这些可观察对象 `onEnterScene` 和 `onLeaveScene` 从 `'@dcl/sdk/observables'` 导入的版本也已弃用。正确的函数名称相同，但应从 `'@dcl/sdk/src/players'` 导入。下面的示例中可以看到两种写法，先是已弃用的版本，然后是正确的版本。

```ts
// 已弃用 - 从 observables 导入
import { onEnterScene, onLeaveScene } from '@dcl/sdk/observables'

onEnterScene.add((player) => {
	console.log('player entered scene: ', player.userId)
})

onLeaveScene.add((player) => {
	console.log('player left scene: ', player.userId)
})

// 当前版本 - 作为玩家函数导入
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)
	})
}
```

## 玩家连接或断开连接


> [!WARNING]
> **📔 注意**： `getConnectedPlayers` 函数和 `onPlayerConnectedObservable` 和 `onPlayerDisconnectedObservable` 事件在 SDK 7.x 中已弃用。请使用 `onEnterScene` 替代，参见 [玩家进入或离开场景](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-enters-or-leaves-scene)。现在每个场景都是一个独立的通信岛，因此处于同一连接状态与处于同一场景的效果相同。


从 `getConnectedPlayers`.

```ts
import { getConnectedPlayers } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getConnectedPlayers({})
	connectedPlayers.players.forEach((player) => {
		console.log('player was already here: ', player.userId)
	})
})
```

每当其他玩家开始或停止被本地引擎渲染时，都会创建一个你可以监听的事件。玩家可能站在与你相同的场景中，也可能不在同一个场景中，但必须处于可视范围内（不一定在视线内）。 `onPlayerConnectedObservable` 会同时检测到玩家在附近新连接或进入可视范围，而 `onPlayerDisconnectedObservable` 则会检测到玩家结束会话或走得很远。

```ts
import {
	onPlayerConnectedObservable,
	onPlayerDisconnectedObservable,
} from '@dcl/sdk/observables'

onPlayerConnectedObservable.add((player) => {
	console.log('player entered: ', player.userId)
})

onPlayerDisconnectedObservable.add((player) => {
	console.log('player left: ', player.userId)
})
```

请记住，如果在玩家加载你的场景之前，周围已经有其他玩家正在被渲染，那么这个事件不会通知新加载的场景这些已经存在的玩家。如果你需要跟踪所有当前玩家，可以在场景加载时查询已有玩家，然后监听此事件以获取更新。

## 查询场景中的所有玩家


> [!WARNING]
> **📔 注意**： `getPlayersInScene` 函数在 SDK 7.x 中已弃用。相反，请通过 `PlayerIdentityData` 组件遍历所有玩家。参见 [获取所有玩家](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#fetch-all-players).


你也可以通过调用 `getPlayersInScene()`.

```ts
import { getPlayersInScene } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getPlayersInScene({})
	connectedPlayers.players.forEach((player) => {
		console.log('player was already here: ', player.userId)
	})
})
```

## 玩家播放动画


> [!WARNING]
> **📔 注意**： `onPlayerExpressionObservable` 事件在 SDK 7.x 中已弃用。请使用 `AvatarEmoteCommand` 组件替代，参见 [玩家播放动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-plays-animation).


每当玩家做出表情动作（跳舞、鼓掌、挥手等）时，你都可以检测到这个事件。

```ts
import { onPlayerExpressionObservable } from '@dcl/sdk/observables'

onPlayerExpressionObservable.add(({ expressionId }) => {
	console.log('Expression: ', expressionId)
})
```

该事件包含以下信息：

* expressionId：执行的表情动作名称（即： *挥手*, *鼓掌*, *亲吻*)


> [!WARNING]
> **📔 注意**：当玩家做出表情且场景已加载时，此事件会被触发。发生时，玩家可能站在附近的其他场景中。


## 玩家更改个人资料


> [!WARNING]
> **📔 注意**： `onProfileChanged` 事件在 SDK 7.x 中已弃用。请使用 `AvatarEquippedData` 组件替代，参见 [玩家更改个人资料](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-changes-profile).


每当玩家对其个人资料进行更改时， `onProfileChanged` 事件就会被调用。这些更改可能包括穿戴不同的服饰、更改名称、描述、启用便携体验等。

```ts
import { onProfileChanged } from '@dcl/sdk/observables'

onProfileChanged.add((profileData) => {
	console.log('Own profile data is ', profileData)
})
```

事件数据只包含玩家 ID 和该头像资料的版本号，依据 catalyst 服务器的记录。每次更改被传播时，版本号都会增加 1。

当此事件被触发时，你可以使用 [getUserData()](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data) 函数获取此信息的最新版本，包括玩家所穿戴的物品列表。你可能需要在调用 `getUserData()` 之前稍作延迟，以确保该函数返回的版本是最新的。


> [!NOTE]
> **💡 提示**：在使用旧版网页浏览器进行预览测试时，为避免使用随机头像，请在与你的 Metamask 钱包连接的浏览器中运行场景。



> [!WARNING]
> **📔 注意**：此事件只会因当前玩家的更改而触发，不会因附近其他玩家资料的更改而触发。


## 场景加载完成


> [!WARNING]
> **📔 注意**： `onSceneReadyObservable` 事件在 SDK v7.x 中已弃用。此函数已不再相关。你可以将需要在场景加载完成后执行的内容放在 `Main()` 函数中执行。参见 [场景生命周期](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#scene-lifecycle)


当场景加载完成时， `onSceneReadyObservable` 会被调用。无论玩家是直接加载进场景，还是从别处走到场景中，这都适用。当场景中的所有内容都完成首次加载后，包括大型模型等，此事件会被调用。

```ts
import { onSceneReadyObservable } from '@dcl/sdk/observables'

onSceneReadyObservable.add(() => {
	console.log('SCENE LOADED')
})
```

## 已弃用的玩家数据方法


> [!WARNING]
> **📔 注意**： `getUserData()` 和 `getPlayerData()` 函数在 SDK v7.4.x 中已弃用。请改用 `getPlayer()` 。参见 [用户数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data).


要获取正在运行场景的当前玩家信息，请使用 `getUserData()`.

下面的示例导入了 `~system/UserIdentity` 命名空间并运行 `getUserData()`.

```ts
import { getUserData } from '~system/UserIdentity'

executeTask(async () => {
	let userData = await getUserData({})
	console.log(userData.data)
})
```

你也可以通过调用 `getPlayerData()`，并传入一个 Decentraland 账户的 ID，来获取附近其他玩家的数据。

```ts
import { getPlayerData } from '~system/Players'

executeTask(async () => {
	let userData = await getPlayerData({ userId: '0x….' })
	console.log(userData)
})
```

这两者都 `getUserData()` 和 `getPlayerData()` 返回与内容 API 中可用的相同数据结构。参见 [任意玩家的数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#data-from-any-player)

`getPlayerData()` 只能获取当前附近玩家的数据。它们不一定必须站在同一个场景中，但必须在可视范围内，因为这些信息是从正在渲染这些头像的本地引擎中获取的。在预览中尝试此功能时，请打开第二个标签页并使用不同的账户登录。


> [!WARNING]
> **📔 注意**：用户 ID 必须始终为小写。如果复制钱包地址，请确保所有字符都为小写。


该 `getUserPublicKey()` 和 `getUserAccount()` 函数也已弃用。请使用 `getPlayer()` 。参见 [用户数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data).

## 获取天空盒时间


> [!WARNING]
> **📔 注意**： `getDecentralandTime()` 函数在 SDK v7.x.x 中已弃用。请使用 `getWorldTime()` 。参见 [获取 Decentraland 时间](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/runtime-data/#get-decentraland-time).


```ts
import { getDecentralandTime } from '~system/EnvironmentApi'

executeTask(async () => {
	let time = await getDecentralandTime({})
	console.log(time)
})
```

## 获取 realm


> [!WARNING]
> **📔 注意**： `getCurrentRealm()` 函数在 SDK v7.x.x 中已弃用。请使用 `getRealm()` 。参见 [获取 Realm 数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/runtime-data/#get-realm-data).


```ts
import { getCurrentRealm } from '~system/EnvironmentApi'

async function fetchPlayerData() {
	const playerRealm = await getCurrentRealm({})

	console.log(playerRealm.currentRealm?.domain)
}

fetchPlayerData()
```

## 是否为预览模式


> [!WARNING]
> **📔 注意**： `isPreviewMode()` 函数在 SDK v7.x.x 中已弃用。请使用 `getRealm()` 替代方案，其中包含一个 `preview` 属性。参见 [获取 Realm 数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/runtime-data/#get-realm-data).


```ts
import { isPreviewMode } from '~system/EnvironmentApi'

executeTask(async () => {
	const { isPreview } = await isPreviewMode({})

	if (isPreview) {
		console.log('Running in preview')
	}
})
```

## 玩家点击另一位玩家

每当玩家点击另一位玩家时，你都可以检测到一个事件。

```ts
import { onPlayerClickedObservable } from '@dcl/sdk/observables'

onPlayerClickedObservable.add((clickEvent) => {
	console.log('Clicked ', clickEvent.userId, ' details: ', clickEvent.ray)
})
```


> [!WARNING]
> **📔 注意**： `onPlayerClickedObservable` 事件在 SDK v7.x 中已弃用。未来版本将支持更 [面向数据的方法](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/)，基于定期查询数据而不是事件。
> 
> 作为替代方案，你可以给玩家附加一个不可见的碰撞体，并通过它检测点击。



> [!WARNING]
> **📔 注意**：执行点击的玩家和被点击的玩家都必须站在场景地块范围内。此监听器只会检测当前玩家点击其他玩家的事件，不会检测其他玩家发起的点击。


该事件包含以下数据：

* `userId`：被点击玩家的 ID
* `ray`：点击射线的相关数据
  * `direction`: *Vector3* 一个归一化的 Vector3，表示从点击原点到点击命中点的方向。
  * `distance`: *number* 从原点到命中点的距离，单位为米。
  * `origin`: *Vector3* 点击的原点，即执行点击的玩家位置，相对于场景。


> [!NOTE]
> **💡 提示**：点击另一位玩家的默认行为是打开玩家护照，你可以在其中查看该玩家的更多信息、将其添加为好友等。你可以通过添加一个 [Avatar Modifier Area](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas/).


## 玩家锁定/解锁光标


> [!WARNING]
> **📔 注意**： `onPointerLockedStateChange` 事件已在 SDK v7.x 中移除，现已不可用。参见 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-locks-or-unlocks-cursor) 以了解当前替代方案。


玩家可以在两种光标模式之间切换： *锁定光标* 模式用于控制摄像机，或使用 *解锁光标* 模式在 UI 上自由移动光标。

玩家可以通过点击 *鼠标右键* 或按下 *Esc* 键来解锁光标，再点击屏幕任意位置即可重新锁定光标。

要在玩家在这两种模式之间切换时做出响应，请使用 `onChange` 函数，作用于 `PointerLock` 组件，参见 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-locks-or-unlocks-cursor).

## 玩家更改 realm 或 island

Decentraland 中的玩家存在于彼此独立的 *realm*中，以及各自独立的 *island* 中。不同 realm 或 island 中的玩家无法看到彼此、与彼此互动或聊天，即使他们站在相同的地块上也是如此。

每次玩家更改 realm 或 island 时， `onRealmChangedObservable` 事件都会被调用。

```ts
import { onRealmChangedObservable } from '@dcl/sdk/observables'

onRealmChangedObservable.add((realmChange) => {
	console.log('PLAYER CHANGED ISLAND TO ', realmChange.room)
})
```


> [!WARNING]
> **📔 注意**： `onRealmChangedObservable` 事件在 SDK v7.x 中已弃用。未来版本将支持更 [面向数据的方法](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/)，基于定期查询数据而不是事件。


此事件包含以下字段：

* **serverName**: *string*；catalyst 服务器名称。
* **room**: *string*；岛屿名称。
* **displayName**: *string*；catalyst 服务器名称后接一个 *-* 以及岛屿名称。例如 `unicorn-x011`.
* **domain**: *string*；所使用的 catalyst 服务器的 URL。

随着玩家在地图中移动，他们可能会切换到新的岛屿，与现在离他们最近的玩家分组。岛屿也会动态调整边界，以便在每个岛屿中容纳一组可管理的人数。因此，即使玩家保持不动，当其他人进入和离开周围场景时，他们也可能被分配到新的岛屿。

如果你的场景依赖于 [第三方服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/third-party-servers/) 来实时同步玩家之间的变化，那么你可能只想在处于同一个 realm+island 的玩家之间共享数据，因此当玩家更改岛屿时，最好在第三方服务器中更改房间。

## 加密函数


> [!WARNING]
> **📔 注意**：函数 `requirePayment()`, `signMessage()`, `convertMessageToObject()` 已弃用。请使用 `sendAsync()` 函数代替。参见 [场景区块链操作](/docs/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/scene-blockchain-operations/)。此外，还有一些库可以帮助简化这些函数的一些常见用例。


## 视频事件


> [!WARNING]
> **📔 注意**： `onVideoEvent` 事件在 SDK v7.x 中已弃用。参见 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#video-events) 以获取非弃用的替代方案。


当视频更改其播放状态时， `onVideoEvent` 可观察对象会接收到一个事件。

```ts
onVideoEvent.add((data) => {
	console.log('New Video Event ', data)
})
```

视频事件的输入包含以下属性：

* `videoClipId` ( *string*）：更改状态的实体的 ID。
* `componentId` (*string*）：更改状态的实体的 ID。
* `currentOffset` (*number*）：视频上 `seek` 属性的当前值。此值显示的是从视频原始开始位置起经过的秒数。 *-1* ，默认情况下。
* `totalVideoLength` (*number* ）：整个视频的长度（秒）。 *-1* 如果长度未知则为。
* `videoStatus`： `VideoTexture`的新视频状态值，以 `VideoStatus` 枚举值表示。该枚举可能包含以下值：
* `VideoStatus.NONE` = 0,
* `VideoStatus.ERROR` = 1,
* `VideoStatus.LOADING` = 2,
* `VideoStatus.READY` = 3,
* `VideoStatus.PLAYING` = 4,
* `VideoStatus.BUFFERING` = 5


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/deprecated-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
