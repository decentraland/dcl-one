> 场景可以跟踪的事件，与玩家动作和场景变化相关。

# 事件监听器

场景可以订阅几个事件，以了解玩家在场景内或附近时的动作。

关于玩家执行的按钮和点击事件，请参见 [按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/).

## 玩家进入或离开场景

每当头像踏入或离开构成你场景的地块，或传送进出时，都会创建一个你可以监听的事件。

这个事件会由所有头像触发，包括玩家自己的头像。

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('进入场景', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('离开场景', userId)
	})
}
```

在 `onEnterScene` 事件中，函数可以访问由 [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data) 通过 `player` 属性。在 `onLeaveScene` 事件中，函数只能访问玩家的 ID。

### 仅当前玩家

你可以过滤触发的事件，只对玩家自己的头像作出反应，而不是周围可能存在的其他头像。

```ts
import { getPlayer, onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	let myPlayer = getPlayer()

	onEnterScene((player) => {
		if (!player) return
		console.log('进入场景', player)

		if (myPlayer && player.userId == myPlayer.userId) {
			console.log('我进来了')
		}
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('离开场景', userId)

		if (myPlayer && userId == myPlayer.userId) {
			console.log('我离开了')
		}
	})
}
```

这个示例先获取玩家的 ID，然后订阅事件，并将事件返回的 `userId` 与玩家的进行比较。

### 查询场景中的所有玩家

通过遍历所有带有 `PlayerIdentityData` 组件的实体，浏览当前在你场景中的完整玩家列表。

```ts
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'

export function main() {
	for (const [entity, data, transform] of engine.getEntitiesWith(
		PlayerIdentityData,
		Transform
	)) {
		console.log('玩家：', { entity, data, transform })
	}
}
```

## 玩家更改摄像机模式

了解摄像机模式非常有助于微调你场景的机制，使其更适合在该模式下更舒适的操作。例如，在第三人称模式下，小目标更难点击。

下面的代码片段使用 `onChange` 函数在每次摄像机变化时触发一个事件。场景加载时，它也会触发一个事件，包含玩家的初始摄像机模式。

```ts
import { engine, CameraMode } from '@dcl/sdk/ecs'

export function main() {
	CameraMode.onChange(engine.CameraEntity, (cameraComponent) => {
		if (!cameraComponent) return
		console.log('摄像机模式已更改', cameraComponent?.mode)
		// 0 = 第一人称
		// 1 = 第三人称
	})
}
```

参见 [检查玩家的摄像机模式](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#check-the-players-camera-mode).

## 玩家播放动画

使用 `onChange` 函数在 `AvatarEmoteCommand` 组件，在玩家每次播放表情动作时触发一个事件。这包括基础表情动作（跳舞、鼓掌、挥手等）以及来自代币的表情动作。

```ts
import { AvatarEmoteCommand } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return
		console.log('已播放表情动作：', emote.emoteUrn)
	})
}
```

该事件包含以下信息：

* `emoteUrn`：所执行表情动作的名称（即： *挥手*, *鼓掌*, *亲吻*)
* `循环`：该表情动作是循环播放还是只播放一次
* `timestamp`：触发表情动作的时间。
* `state`： `EmoteState` 枚举中的一个值，用于描述生命周期事件： `ES_STARTED` （表情动作已开始，同时也是该字段缺失时的值）， `ES_FINISHED` （非循环表情动作已播放至结束），或 `ES_INTERRUPTED` （表情动作因移动、传送、其他表情动作、显式停止或离开场景而被中断）。参见 [检测表情动作何时结束](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#detect-when-an-emote-finishes).

你也可以通过传入其他玩家的引用，而不是 `engine.PlayerEntity`.


> [!NOTE]
> **💡 提示**：有关表情动作播放事件的可运行示例，请参见 [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) 测试场景，它会记录每一条 `AvatarEmoteCommand` 附加到玩家实体上的条目，作为 `STARTED`, `FINISHED` 或 `INTERRUPTED` ——完整播放一个表情动作以查看 `FINISHED`，或者在播放中途走开以查看 `INTERRUPTED`.


## 玩家更改资料

使用 `onChange` 函数在 `AvatarEquippedData` 组件，在玩家每次更换一件穿戴物，或快速访问轮上的已列出表情动作时触发一个事件。同样，使用 `onChange` 函数在 `AvatarBase` 来在玩家每次更改其基础头像属性时触发一个事件，例如发色、肤色、头像形状或名称。

```ts
import { AvatarEquippedData, AvatarBase } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('新的穿戴物列表：', equipped.wearableUrns)
		console.log('新的表情动作列表：', equipped.emoteUrns)
	})

	AvatarBase.onChange(engine.PlayerEntity, (body) => {
		if (!body) return
		console.log('新的眼睛颜色：', body.eyesColor)
		console.log('新的肤色：', body.skinColor)
		console.log('新的头发颜色：', body.hairColor)
		console.log('新的身体 URN：', body.bodyShapeUrn)
	})
}
```

在 `AvatarEquippedData` 包含以下信息：

* `wearableUrns`：玩家当前已装备的穿戴物列表。
* `emoteUrns`：玩家当前在快速访问轮上已装备的表情动作列表。

在 `AvatarBase` 包含以下信息：

* `name`：玩家的名称。
* `bodyShapeUrn`：对应男性或女性体型的 ID。
* `skinColor`：玩家肤色，类型为 `Color3`
* `eyesColor`：玩家眼睛颜色，类型为 `Color3`
* `hairColor`：玩家头发颜色，类型为 `Color3`
* `talking`: *true* 当玩家在语音聊天中说话时， *false* 当他们沉默时， `undefined` 当客户端未报告语音活动时。

因为 `talking` 位于 `AvatarBase`，因此这个回调也会在玩家开始或停止说话时触发。如果你只关心外观，请比较你需要的字段，而不要对每次变化都作出反应。参见 [检测玩家何时在说话](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#detect-when-a-player-is-speaking).

你也可以检测场景中其他玩家的穿戴物或头像变化，只需传入其他玩家的引用，而不是 `engine.PlayerEntity`.

你也可以检测场景中其他玩家资料的变化，只需传入其他玩家的引用，而不是 `engine.PlayerEntity`.

## 玩家锁定或解锁光标

玩家可以在两种光标模式之间切换： *锁定光标* 模式，用于控制摄像机，或 *解锁光标* 模式，用于在 UI 上自由移动光标。

玩家通过点击 *鼠标右键* 或按下 *Esc* 键解锁光标，并通过点击屏幕任意位置将光标重新锁定。

添加一个 `PointerLock` 组件到 `engine.CameraEntity` 实体上，并使用 `onChange` 函数在 `PointerLock` 组件在玩家在两种光标模式之间切换时触发一个事件。

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    // 显示有关光标模式的提示
		   }
	})
}
```

你可以利用这些信息轻微引导玩家，例如显示一个 UI 弹窗，提示这款游戏更适合在解锁光标模式下体验。或者你也可以通过更改 `isPointerLocked` 组件上的该值来强制玩家的光标模式。下面的示例始终将光标模式设置为解锁：

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false});
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    PointerLock.getMutable(engine.CameraEntity).isPointerLocked = false
		   }
	})
}
```

## 打开 Explorer 面板

你的场景可以使用 Explorer 内置面板（地图、设置、背包等）来 `openExplorerUi` 受限操作来打开。它们中的大多数会占据整个屏幕，但并非全部都会。这对于从你场景的 UI 引导玩家使用特定的 Explorer 功能很有用。

必须由玩家发起。调用 `openExplorerUi` 要在点击处理函数内部调用，而不是单独调用：

```ts
// 当玩家点击立方体时打开地图
import { openExplorerUi } from '~system/RestrictedActions'
import { engine, ExplorerUi, InputAction, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const mapButton = engine.addEntity()
Transform.create(mapButton, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(mapButton)
MeshCollider.setBox(mapButton)

pointerEventsSystem.onPointerDown(
	{ entity: mapButton, opts: { button: InputAction.IA_POINTER, hoverText: '打开地图' } },
	() => {
		void openExplorerUi({ ui: ExplorerUi.EU_MAP })
	}
)
```

该 `ExplorerUi` 枚举具有以下值：

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)
* `EU_ITEM_PURCHASE` （7）：购买物品确认模态框。Explorer 当前返回 `REJECTED_FEATURE_DISABLED` ，表示该功能已禁用。


> [!WARNING]
> **注意：** 与其他受限操作（`movePlayerTo`, `triggerEmote`), `openExplorerUi` 一样，只有在玩家站在场景地块内时才有效。


### 只有玩家动作才能打开面板

只有当调用是对玩家刚刚执行的操作的直接响应时，面板才会打开。否则该调用会被拒绝。

**可行：**

* 使用 `pointerEventsSystem.onPointerDown` 或 `pointerEventsSystem.onPointerUp`.
* 点击你场景 UI 中的元素，使用 `onMouseDown`.
* 按下或释放其中一个激活按钮， `E`, `F` 或 `1` 到 `4`，通过以下方式检测： `inputSystem.isTriggered` 和 `PointerEventType.PET_DOWN` 或 `PointerEventType.PET_UP` (`IA_PRIMARY`, `IA_SECONDARY`, `IA_ACTION_3` 到 `IA_ACTION_6`).

**不可行：**

* 从 `main()`调用，因此面板无法在加载时向玩家致意。
* 从计时器中调用，或者在玩家动作已经过去之后再调用。
* 在一个 `await` 对需要时间的操作，例如 `fetch` 或其他受限操作之后调用。玩家动作大约只在一帧内有效。先完成工作，再打开面板。
* 悬停。 `PET_HOVER_ENTER` 和 `PET_HOVER_LEAVE` 不是玩家动作。
* 移动按键。走路、奔跑和跳跃都不算，只有上面列出的激活按钮才算。

`openExplorerUi` 会返回一个 `openResult` ，告诉你发生了什么：

| 值                                | 含义                                   |
| -------------------------------- | ------------------------------------ |
| `UNSPECIFIED` (0)                | 未报告结果。                               |
| `OPENED` (1)                     | 面板已打开。                               |
| `WAS_ALREADY_OPEN` (2)           | 屏幕上已经有另一个面板。所有面板共享一个槽位，因此同一时间只能打开一个。 |
| `REJECTED_NOT_CURRENT_SCENE` (3) | 玩家不在你的场景中。                           |
| `REJECTED_FEATURE_DISABLED` (4)  | Explorer 已关闭该面板，或者 `ui` 该值不是它识别的面板。  |
| `REJECTED_NO_USER_GESTURE` (5)   | 该调用不是对玩家动作的响应。                       |


> [!WARNING]
> **注意：** 如果没有任何内容打开且你看不到错误，请检查 `openResult`. `REJECTED_NO_USER_GESTURE` （5）表示你的调用在玩家动作之外执行，通常是因为它位于 `main()`、在计时器中，或者在一个 `await`.


`WAS_ALREADY_OPEN` 这不仅仅是因为你的场景调用了两次。所有面板都占用同一个槽位，因此任何一个面板在屏幕上都会拒绝其他面板，而且玩家可能在刚刚之前手动打开了那个面板，而与你的场景无关。请将其视为正常结果，而不是你代码中的错误。使用 `openExplorerUiAndWait()`，如下所述，相同情况会以 `notOpened` 结果返回。

### 等待玩家关闭面板

`openExplorerUiAndWait()` 会打开一个面板，并返回一个承诺，该承诺会在玩家关闭它后解决。当你的场景应只在玩家完成操作后继续时使用它，例如在把玩家送到背包去穿戴某个物品之后。

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// 打开背包，然后在玩家关闭后继续
async function openBackpack() {
	try {
		const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_BACKPACK })

		if (outcome.$case === 'closed') {
			console.log('玩家已关闭背包')
		}
	} catch (error) {
		console.log('等待失败：', error)
	}
}
```

调用 `openBackpack()` 从点击处理函数中，就像 `openExplorerUi`。 `await` 发生在函数内部，在请求面板之后，因此玩家动作仍然有效。

你返回得到的对象有一个 `$case` 字段，用于告诉你等待是如何结束的：

* `closed`：玩家关闭了你场景打开的面板。这是正常结果。
* `notOpened`：没有打开任何内容。 `openResult` 字段会使用上表中的值说明原因。
* `timedOut`：你设置的时间限制在面板仍然打开时到期了。
* `matched`：一个 `until` 条件提前结束了等待，因此面板可能仍在屏幕上。

当面板无法打开时，承诺会立即返回 `notOpened`。你的场景不会一直等待一个并不存在的面板。

这四种结果涵盖了所有正常结束情况，但 promise 仍然可能被拒绝：对 Explorer 的请求可能直接失败，两个频道可能以相同的名称注册，或者可能到来一个 helper 无法将其归因于任何正在进行中的调用的事件。这就是为什么上面的示例被包裹在一个 `try`/`catch`中。在一个 `async` 函数中发生的拒绝，如果它是从指针处理程序调用的，就根本不会被报告，所以如果没有 `catch` 等待就会一直没有返回，而且没有任何东西告诉你原因。


> [!WARNING]
> **📔 注意**: `openExplorerUiAndWait()` 需要 `@dcl/sdk` 7.30.0 版或更新版本。它来自 `@dcl/sdk/explorer-ui` 模块，该模块还重新导出 `ExplorerUi`.


#### 设置时间限制

默认没有时间限制。玩家可以让面板保持打开几分钟，而 promise 在这段时间里一直处于待定状态。传入 `timeoutMs` ，当你的场景无论如何都需要继续推进时。

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// 无论地图是否还打开，30 秒后停止等待
async function openMap() {
	const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_MAP }, { timeoutMs: 30000 })

	if (outcome.$case === 'timedOut') {
		console.log('玩家仍在查看地图')
	}
}
```

#### 改为在某个事件上停止

该 `until` 选项会在某个事件到达频道时立即结束等待，并返回 `matched` 而不是 `closed`。频道由同一个模块导出： `ExplorerUiEvents` 用于面板打开和关闭事件，以及 `ItemPurchase` 用于购买流程的结果。也把同一个频道添加到 `collect` 选项中；如果你还想要事件本身，它们会出现在结果的 `events` 数组中。

单独传入该频道即可在其任意事件上停止，或者将其包裹在 `variant()` 中，仅在一种事件上停止：

```ts
import { ExplorerUi, ExplorerUiEvents, openExplorerUiAndWait, variant } from '@dcl/sdk/explorer-ui'

// 当背包出现在屏幕上就继续，而不等待玩家关闭它
async function openBackpack() {
	const outcome = await openExplorerUiAndWait(
		{ ui: ExplorerUi.EU_BACKPACK },
		{ until: variant(ExplorerUiEvents, 'opened'), collect: [ExplorerUiEvents] }
	)

	if (outcome.$case === 'matched') {
		console.log('背包已打开，收集到', outcome.events.length, '个事件')
	}
}
```

该 `$case` 值 `variant()` 可接受的值就是该频道事件携带的值： `opened` 和 `closed` on `ExplorerUiEvents`.

### 观察面板何时打开或关闭

使用 `ExplorerUiEventsResult` 组件，用于跟踪你自己的场景打开的面板。这是一个仅增长组件，位于 `engine.RootEntity`，类似于 `PointerEventsResult`。你的场景使用以下方式打开的每个面板 `openExplorerUi` 在出现时会添加一个事件，在消失时会再添加一个事件，包括玩家通过以下方式将其关闭时 *Esc*.


> [!WARNING]
> **注意：** 这并不是整个 Explorer UI 的场景级视图。玩家通过 Explorer 自己的菜单手动打开的面板，不会在这里产生任何事件。一个从未调用 `openExplorerUi` 的场景一个也看不到。


`get()` 会把到目前为止累积的全部事件集交给你，最多保留最近 100 条，而不是自上一帧以来到达的那些。一个盲目遍历它的系统会在每个 tick 上响应所有过去的事件。将每个事件的 `timestamp` 与你处理过的上一个 tick 进行比较：

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

let lastHandledTick = -1

engine.addSystem(() => {
	let newestTick = lastHandledTick

	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		// 该集合保留所有事件，因此跳过已处理过的 tick
		if (event.timestamp <= lastHandledTick) continue
		if (event.timestamp > newestTick) newestTick = event.timestamp

		if (event.event?.$case === 'opened') {
			console.log('面板已打开：', event.ui)
		}
		if (event.event?.$case === 'closed') {
			console.log('面板已关闭：', event.ui)
		}

		// 检查具体是哪个面板
		if (event.ui === ExplorerUi.EU_MAP && event.event?.$case === 'closed') {
			console.log('玩家关闭了地图')
		}
	}

	// 只有在循环结束后，这样同一 tick 上的多个事件都会被处理
	lastHandledTick = newestTick
})
```

`ExplorerUiEventsResult.onChange()` on `engine.RootEntity` 是读取该流的另一种方式：回调会针对每个新事件运行一次，因此无需维护这些状态。

每个事件包含：

* `ui`：该事件涉及哪个面板（一个 `ExplorerUi` 值）。
* `timestamp`：该事件发生时的场景 tick。
* `requestId`：你的场景在以下调用中传入的 id `openExplorerUi` 打开此面板的调用，或者 `0` 如果调用未传入任何 id。可用它来区分多个正在进行的调用。
* `event`：要么是 `{ $case: 'opened' }` 或 `{ $case: 'closed' }`.


> [!NOTE]
> **💡 提示**：有关以上所有内容的可运行示例，请查看 [`80,-6-explorer-ui-events`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/80,-6-explorer-ui-events) 测试场景。它会把每个判定和每个事件打印到自己的 HUD 上，并涵盖手势门禁、一个已打开的面板， `requestId` 关联，以及以下各项的每种结果： `openExplorerUiAndWait()`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
