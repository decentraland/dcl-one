> 将玩家传送到场景中的新位置，或引导其步行到新位置

# 移动玩家

要更改场景中玩家的位置，请使用 `movePlayerTo()` 函数。可用于让玩家重新生成、设置检查点，或让玩家在过场动画中走到某个位置。


> [!NOTE]
> **💡 提示**：移动玩家最简单的方法是使用 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)。使用无代码 **移动玩家** 或 **将玩家移动到此处** 操作，参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).


此函数接受一个包含以下属性的对象：

* `newRelativePosition`：玩家要定位到的位置，以 Vector3 表示。
* `cameraTarget`：（可选）让摄像机朝向的方向，以 Vector3 表示，用于表示要注视的空间点坐标。如果未提供值，摄像机将保持与移动前相同的旋转。
* `avatarTarget`：（可选）让头像朝向的方向，以 Vector3 表示，用于表示要注视的空间点坐标。如果未提供值，头像将保持与移动前相同的旋转。如果玩家处于第一人称摄像机模式，摄像机和头像的旋转相同。
* `duration`：（可选）过渡应持续的时间，单位为秒。如果未提供值，过渡将立即发生。如果提供了持续时间，头像将走或跑到这个新位置。


> [!WARNING]
> **📔 注意**：在过渡期间，头像不受碰撞体影响，因此可以穿过物体。


```ts
import { movePlayerTo } from '~system/RestrictedActions'

// 创建实体
const myEntity = engine.addEntity()
MeshRenderer.setBox(myEntity)
MeshCollider.setBox(myEntity)

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

// 赋予实体行为
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '点击' },
	},
	function () {
		// 重生玩家
		movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
		})
	}
)
```

玩家的移动会立即发生，不会出现任何确认界面或摄像机过渡。


> [!WARNING]
> **📔 注意**：只有当玩家已经站在场景边界内时，才能移动他们；并且只能移动到场景边界范围内的位置。你不能使用 `movePlayerTo()` 来将玩家传送到另一个场景。要将玩家移动到另一个场景，请参见 [传送](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/external-links/#teleports).


该 `movePlayerTo()` 函数是可等待的，因此如果移动有持续时间，你可以使用 `await` 来等待玩家到达目的地。

```ts
import { movePlayerTo } from '~system/RestrictedActions'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '点击' },
	},
	async function () {
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})
	}
)
```


> [!WARNING]
> **📔 注意**：如果没有 `duration`，Promise 会在传送被加入队列后立即解析，而不是在玩家到达后才解析。位于 `await` 之后的代码会在 `Transform.get(engine.PlayerEntity)` 仍然报告旧位置时运行，而稍后落地的移动可能会打断你在那里开始的表情动作。如果有事情必须在到达时发生，请改为轮询玩家的位置：
> 
> ```ts
> import { engine, Transform } from '@dcl/sdk/ecs'
> import { Vector3 } from '@dcl/sdk/math'
> import { movePlayerTo } from '~system/RestrictedActions'
> 
> function teleportThen(target: Vector3, onArrive: () => void) {
> 	void movePlayerTo({ newRelativePosition: target })
> 
> 	let waited = 0
> 	function waitForArrival(dt: number) {
> 		waited += dt
> 		const pos = Transform.getOrNull(engine.PlayerEntity)?.position
> 		// 只比较 X 和 Z 轴，这样高度差不会让等待卡住
> 		const arrived = pos !== undefined && Math.hypot(pos.x - target.x, pos.z - target.z) < 0.2
> 		if (!arrived && waited < 1.5) return // 1.5 秒后放弃
> 		engine.removeSystem(waitForArrival)
> 		onArrive()
> 	}
> 
> 	engine.addSystem(waitForArrival)
> }
> ```
> 


如果玩家在过渡持续期间尝试移动，则过渡将被中断，并且 `movePlayerTo` 函数将永远不会被解析。你可以在过渡进行时使用 `InputModifier` 组件来禁用所有输入，从而避免这种行为，参见 [限制移动](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#restrict-locomotion).

```ts
import { movePlayerTo } from '~system/RestrictedActions'
import {InputModifier, engine} from '@dcl/sdk/ecs'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '点击' },
	},
	async function () {

		// 禁用所有输入
		InputModifier.create(engine.PlayerEntity, {
			mode: InputModifier.Mode.Standard({
				disableAll: true,
			}),
		})

		// 移动玩家
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})

		// 启用所有输入
		InputModifier.deleteFrom(engine.PlayerEntity)
	}
)
```


> [!WARNING]
> **📔 注意**：此外，await 的结果可以被保存，以判断其 `成功` 属性是否表示移动被中断（例如因输入导致移动）或成功完成。



> [!NOTE]
> **💡 提示**：要将玩家移动到另一个场景，或 Genesis City 地图中的其他坐标，请参见 [传送](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/external-links/#teleports).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/move-player.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
