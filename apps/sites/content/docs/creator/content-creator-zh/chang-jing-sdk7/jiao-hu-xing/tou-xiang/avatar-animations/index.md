> 让玩家的头像播放默认或自定义动画，并检测某个表情动作何时结束

# 头像动画

你可以让玩家在场景代码中执行一个动画。这有助于增强沉浸感，也有助于让其他玩家彼此了解正在做什么。角色动画会同时被该玩家（以第三人称视角）和周围的其他任何玩家看到。

由玩家控制的全身动画会被默认的移动动画覆盖，比如行走和跳跃。因此，场景播放的全身动画只会在玩家站立不动时播放。如果玩家行走或跳跃，任何全身动画都会被中断。上半身动画不会因移动而中断。


> [!WARNING]
> **📔 注意**：只有当玩家已经站在场景边界内时，才能为其播放动画；如果他们在相邻场景中，则不行。智能穿戴物可以在任何地方播放动画。
> 
> 当玩家正在执行动画时，他们不会受到碰撞影响，移动也不会受场景物理规则限制。另外请注意，如果动画使玩家偏离其原始位置（例如动画包含跳跃），玩家的 Transform 组件不会受到这种位移的影响。


### 使用场景编辑器

让玩家执行动画最简单的方法是使用场景编辑器。使用无代码的 **播放表情动作** 操作来播放默认动画，或者使用 **播放自定义表情动作** 操作来播放文件中的动画。参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).

### 默认动画

使用 `triggerEmote()` 函数来运行玩家能够在 Decentraland 中任何地方播放的默认动画之一。该函数接受一个具有以下属性的对象作为参数：

* `predefinedEmote`：已有表情动作的字符串名称。
* `mask`：可选。使用来自 `AvatarMask` 枚举的值，仅在角色身体的某一部分播放动画。例如， `AvatarMask.AM_UPPER_BODY` 只会让角色的上半身动起来。参见 [仅让上半身动起来](#animate-only-the-upper-body).

```ts
import { triggerEmote } from '~system/RestrictedActions'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: '跳舞' },
	},
	() => {
		triggerEmote({ predefinedEmote: 'robot' })
	}
)
```

以下表情动作会反馈你场景中的玩家动作，下面这些都是该字段的有效值： `predefinedEmote` 字段：

* `buttonDown`
* `buttonFront`
* `getHit`
* `knockOut`
* `lever`
* `openChest`
* `openDoor`
* `punch`
* `push`
* `swingWeaponOneHand`
* `swingWeaponTwoHands`
* `throw`
* `sittingChair1`
* `sittingChair2`
* `sittingGround1`
* `sittingGround2`

这四个 `坐姿` 表情动作会循环：角色会保持该姿势，直到玩家移动，或者你的场景调用 `stopEmote({})`. `stopEmote` 返回结果为 `{ success: boolean }`，其值为 `false` ，例如当 Explorer 拒绝了调用时，因为玩家已经不在你的场景中。其他所有动作都会播放一次，然后角色返回待机状态。

这些表情动作对所有玩家都可在其默认表情轮盘中使用，也可用于任何场景。

* `挥手`
* `振臂欢呼`
* `机器人`
* `举手`
* `鼓掌`
* `钱`
* `亲吻`
* `tik`
* `锤子`
* `tektonik`
* `看不见`
* `双手举空`
* `耸肩`
* `迪斯科`
* `dab`
* `头爆炸`
* `制作`
* `双手举高`
* `胜利`
* `挥手`
* `振臂欢呼_short`


> [!WARNING]
> **📔 注意**：请严格按列出的名称使用。名称错误在你的场景中会静默失败：什么都不会播放，场景控制台也不会显示任何内容。注意不要把 `双手举高` 与 `振臂欢呼_short` 连在一起，也不要省略其后缀。



> [!NOTE]
> **💡 提示**：如果玩家在播放动画时行走或跳跃，就会打断动画。如果你不希望这种情况发生，可以在角色动画期间使用 [输入修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#freeze-the-player) 来冻结角色。


### 自定义动画

使用 `triggerSceneEmote()` 来让玩家执行自定义动画，该动画作为场景资源的一部分，存储为 .glb 文件。


> [!NOTE]
> **💡 提示**：你不必自己制作这些动画。 **角色动画** 包在场景编辑器中包含 45 个预制动画，包括坐姿、钓鱼、搬箱子和跳舞。将其中一个拖到 3D 视图中，编辑器会把它的文件复制到你的场景中，然后把该文件路径传给 `triggerSceneEmote()`。文件会放在 `assets/asset-packs/<animation-folder>/`下，例如 `assets/asset-packs/whack/Whack_Avatar_emote.glb`。请在 **本地资源**中查看准确路径。参见 [添加物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/#avatar-animations).



> [!WARNING]
> **📔 注意**：文件名 **必须** 以 `_emote.glb` 结尾，才能作为角色动画使用。


该函数接受一个具有以下属性的对象：

* `src`：表情动作文件的完整路径，从项目根文件夹开始计算。位于 `assets/animations/sit_emote.glb` 的文件需要 `'assets/animations/sit_emote.glb'`.
* `loop`：如果为 true，动画会持续循环播放，直到玩家移动或动画被停止。默认值为 false。
* `mask`：可选。使用来自 `AvatarMask` 枚举的值，仅在角色身体的某一部分播放动画。例如， `AvatarMask.AM_UPPER_BODY` 只会让角色的上半身动起来。参见 [仅让上半身动起来](#animate-only-the-upper-body).


> [!WARNING]
> **📔 注意**: `src` 必须是文件的完整路径。不会按文件名搜索，也不会自动为你添加 `_emote` 后缀。如果路径错误， `triggerSceneEmote()` 返回 `{ success: false }` ，且不会播放任何内容。请从项目文件夹中复制该路径，并包含其中的每个文件夹。


```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import { AvatarMask } from '@dcl/sdk/ecs'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: '制作雪球' },
	},
	() => {
		triggerSceneEmote({ src: 'assets/animations/Snowball_Throw_emote.glb', loop: false, mask: AvatarMask.AM_UPPER_BODY })
	}
)
```


> [!NOTE]
> **💡 提示**：如果玩家在播放动画时行走或跳跃，就会打断动画。如果你不希望这种情况发生，可以在角色动画期间使用 [输入修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#freeze-the-player) 来冻结角色。


#### 道具、粒子和声音

使用 `triggerSceneEmote()` 播放的表情动作文件可以包含道具，例如网球拍或火焰喷射效果，并与角色一起动画播放。按照 Emotes 2.0 表情动作的制作方式构建，并遵循 [道具和声音](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/props-and-sounds/):

* 中的命名规则。 `_Avatar` 和 `_Prop` ，分别用于角色和道具骨架。
* 两个动画片段长度相同。
* 如果你把粒子效果转换为动画骨骼，它们也可以工作，参见 [表情动作中的粒子](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/particles-in-emotes/).

周围的其他玩家会看到这些道具，就像他们看到角色动画一样。


> [!WARNING]
> **📔 注意**: `triggerSceneEmote()` 不会播放声音。音频只会在 Builder 中作为独立表情动作上传的动作里播放。要把声音与场景表情动作配对，请同时使用一个 [AudioSource](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/) 来播放。


### 仅让上半身动起来

二者都 `triggerEmote()` 和 `triggerSceneEmote()` 都接受一个可选的 `mask` 属性，用来限制动画只作用于角色身体的一部分。

通常，动画会在玩家一走动或跳跃时立即停止，因为默认的移动动画会接管整个身体。当你把 `mask` 设置为 `AvatarMask.AM_UPPER_BODY`时，动画只会驱动角色腰部以上的部分，而腿部仍由默认的移动动画控制。这意味着玩家在上半身播放你的动画时，仍然可以在场景中继续走动或奔跑。请将其用于不应中断移动的动作，例如搬箱子、举火把或抛接杂耍。

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import {
	engine,
	AvatarMask,
	InputAction,
	MeshCollider,
	MeshRenderer,
	pointerEventsSystem,
	Transform,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const cheerButton = engine.addEntity()
Transform.create(cheerButton, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(cheerButton)
MeshCollider.setBox(cheerButton)
pointerEventsSystem.onPointerDown(
	{
		entity: cheerButton,
		opts: { button: InputAction.IA_POINTER, hoverText: '欢呼' },
	},
	() => {
		triggerSceneEmote({
			src: 'assets/animations/Cheer_emote.glb',
			loop: true,
			mask: AvatarMask.AM_UPPER_BODY,
		})
	}
)
```

在这个示例中，玩家点击一个按钮并开始欢呼，双手举在空中，动画会循环播放。由于只有上半身在动画化，他们在欢呼时仍然可以在场景中继续奔跑，例如跟随一场比赛或足球赛的进程。

有几点需要注意：

* `AvatarMask.AM_UPPER_BODY` 目前是 `AvatarMask` 枚举中的唯一值。要播放全身动画，只需不要设置 `mask` 属性。
* 该 `loop` 属性的行为与全身动画相同：当 `loop: false` 时，受遮罩的动画会播放一次，然后上半身恢复正常移动；当 `loop: true` 时，它会重复播放直到被停止。
* 要从代码中停止循环播放的遮罩动画，请调用 `stopEmote({})`，它也从 `~system/RestrictedActions`中导入。它返回的结果为 `{ success: boolean }`.

### 检测表情动作何时结束

每个表情动作的生命周期事件都会通过玩家实体上的 `AvatarEmoteCommand` 组件进行报告。每个新条目都带有一个 `state` 字段，其值来自 `EmoteState` 枚举：

* `EmoteState.ES_STARTED`：表情动作已开始播放。当 `state` 字段不存在时，也会报告这个值。
* `EmoteState.ES_FINISHED`：一个非循环播放的表情动作已自然播放结束。
* `EmoteState.ES_INTERRUPTED`：表情动作被提前打断：玩家移动或跳跃、传送、开始了另一个表情动作、明确停止了该表情动作，或者玩家离开了场景。

这既适用于场景触发的表情动作（`triggerEmote()` 和 `triggerSceneEmote()`），也适用于玩家通过表情轮盘自行播放的表情动作，还适用于场景中其他玩家播放的表情动作。

使用 `onChange` 上的 `AvatarEmoteCommand` 函数，以响应每个新条目：

```ts
import { AvatarEmoteCommand, EmoteState } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return

		switch (emote.state ?? EmoteState.ES_STARTED) {
			case EmoteState.ES_STARTED:
				console.log('表情动作已开始：', emote.emoteUrn)
				break
			case EmoteState.ES_FINISHED:
				console.log('表情动作自然结束：', emote.emoteUrn)
				break
			case EmoteState.ES_INTERRUPTED:
				console.log('表情动作被打断：', emote.emoteUrn)
				break
		}
	})
}
```


> [!WARNING]
> **📔 注意**：此功能目前仅支持桌面客户端。使用 `mask` （部分身体表情动作）在本地玩家身上播放时，目前不会报告生命周期事件。



> [!NOTE]
> **💡 提示**：要在不是玩家的角色上播放动画，请参见 [NPC 角色](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/#animations).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
