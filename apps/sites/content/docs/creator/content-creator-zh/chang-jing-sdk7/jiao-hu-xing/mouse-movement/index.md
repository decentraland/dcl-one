> 实时响应玩家的鼠标移动，用于驱动拖拽手势、旋转物体或控制自定义摄像机。

# 鼠标移动

你的场景可以在每一帧读取玩家的原始鼠标移动，并用它驱动实时交互：拖拽和滑动手势、在玩家拖动物体时让物体旋转或滑动，或者像第一人称射击游戏那样控制自定义摄像机。

为此，请读取 `screenDelta` 属性 `PrimaryPointerInfo` 组件中的 `engine.RootEntity`。此属性是一个 `Vector2` ，它报告鼠标自上一帧以来移动了多少像素。正的 `x` 值表示鼠标向右移动，正的 `y` 值表示鼠标向下移动，与屏幕左上角为原点的坐标系一致——与 `UiTransform` 和 `UiCanvasInformation`。在鼠标不移动的帧中，这两个值都为 0。

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta) return
	console.log(`mouse moved: ${delta.x}, ${delta.y}`)
})
```

由于 `screenDelta` 只保存单帧的移动量，因此总是在 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/)中读取它，这样就不会在帧与帧之间漏掉任何移动。


> [!WARNING]
> **📔 注意**：避免在初始场景加载时引用 `engine.RootEntity` ，因为如果实体尚未初始化，可能会导致错误。为避免此问题，请始终在系统中引用该实体。它总是可用，因为系统的首次执行会在场景已经正确初始化后才开始。



> [!WARNING]
> **📔 注意**：本文档中描述的内容仅适用于桌面端玩家。在 [移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)中，输入基于触摸，没有自由移动的光标，因此 `screenDelta` 总是报告 0。参见 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/) 了解触控设备上的输入工作方式，以及 [检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) ，为移动玩家提供替代控制方案。


## 光标锁定时的鼠标移动

`screenDelta` 与 `PrimaryPointerInfo` 的其他属性行为不同，当光标 [锁定](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#lock-or-unlock-the-cursor): `screenCoordinates` 会冻结在屏幕中央，并且 `worldRayDirection` 始终报告屏幕中心的射线，但 `screenDelta` 仍会在每一帧报告原始鼠标移动。这使它成为在光标锁定时读取鼠标移动的唯一方式，而这正是自定义摄像机或瞄准控制所需要的。

## 拖动以旋转物体

以下示例允许玩家按住指针按钮并横向拖动鼠标来旋转立方体：

```ts
import {
	engine,
	InputAction,
	inputSystem,
	MeshCollider,
	MeshRenderer,
	PrimaryPointerInfo,
	Transform,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const DRAG_SENSITIVITY = 0.5

export function main() {
	const cube = engine.addEntity()
	Transform.create(cube, { position: Vector3.create(8, 1, 8) })
	MeshRenderer.setBox(cube)
	MeshCollider.setBox(cube)

	engine.addSystem(() => {
		// 仅在按住指针按钮时旋转
		if (!inputSystem.isPressed(InputAction.IA_POINTER)) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta || delta.x === 0) return

		const transform = Transform.getMutable(cube)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromEulerDegrees(0, delta.x * DRAG_SENSITIVITY, 0)
		)
	})
}
```

同样的模式适用于任何拖拽交互：使用 `delta.x` 作为偏移量沿轨道移动实体、拖动播放动画，或者通过检查较大的 delta 值来检测快速滑动。

## 鼠标视角摄像机控制

因为 `screenDelta` 在光标锁定时仍然有效，你可以用它为 [虚拟摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#using-virtual-cameras)构建鼠标视角控制。为此，将鼠标移动累积到偏航和俯仰角中，然后在每一帧将它们应用到摄像机的旋转上。

在下面的示例中，玩家点击一个盒子切换到可用鼠标控制的虚拟摄像机，并按下次要按钮（*F* 或右键单击）以返回默认摄像机。虚拟摄像机激活时，场景还会锁定光标，并通过 [输入修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#freeze-the-player)冻结玩家角色，这样玩家在控制摄像机时就不会盲目行走。

```ts
import {
	engine,
	Entity,
	InputAction,
	InputModifier,
	inputSystem,
	MainCamera,
	MeshCollider,
	MeshRenderer,
	PointerEventType,
	pointerEventsSystem,
	PointerLock,
	PrimaryPointerInfo,
	Transform,
	VirtualCamera,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// 每像素鼠标移动对应的摄像机旋转角度
const SENSITIVITY = 0.15

let cameraEntity: Entity
let cameraActive = false
let yaw = 0
let pitch = 0

export function main() {
	// 摄像机跟随的实体
	cameraEntity = engine.addEntity()
	Transform.create(cameraEntity, { position: Vector3.create(8, 3, 8) })
	VirtualCamera.create(cameraEntity, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0.5) },
	})

	// 点击此盒子进入鼠标视角模式
	const box = engine.addEntity()
	Transform.create(box, { position: Vector3.create(8, 1, 4) })
	MeshRenderer.setBox(box)
	MeshCollider.setBox(box)
	pointerEventsSystem.onPointerDown(
		{
			entity: box,
			opts: { button: InputAction.IA_POINTER, hoverText: '控制摄像机' },
		},
		() => activateCamera(true)
	)

	// 使用鼠标控制摄像机
	engine.addSystem(() => {
		if (!cameraActive) return
		if (!PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta) return

		yaw += delta.x * SENSITIVITY
		// 限制俯仰角，避免摄像机翻转
		pitch = Math.max(-85, Math.min(85, pitch + delta.y * SENSITIVITY))

		Transform.getMutable(cameraEntity).rotation = Quaternion.fromEulerDegrees(pitch, yaw, 0)
	})

	// 使用次要按钮退出鼠标视角模式
	engine.addSystem(() => {
		if (!cameraActive) return
		if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) {
			activateCamera(false)
		}
	})
}

function activateCamera(active: boolean) {
	cameraActive = active

	// 分配或释放虚拟摄像机
	MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: active ? cameraEntity : undefined,
	})

	// 在控制摄像机时冻结玩家角色
	InputModifier.createOrReplace(engine.PlayerEntity, {
		mode: InputModifier.Mode.Standard({ disableAll: active }),
	})

	// 锁定光标，这样鼠标就能立即控制摄像机
	PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: active })
}
```

关于此示例，有几点需要注意：

* 系统将 `delta.y` 添加到俯仰角。向上移动鼠标会报告一个负的 `delta.y`，这会降低俯仰角并让摄像机向上倾斜。如果你更喜欢反转的垂直控制，请将该符号取反。
* 俯仰角被限制在 -85 到 85 度之间，因此摄像机绝不会向后翻转。
* 该 `SENSITIVITY` 常量表示每像素鼠标移动对应的旋转角度，可按需调整。


> [!NOTE]
> **💡 提示**：玩家可以随时通过按下 *Esc* 或右键单击来解锁光标，这会停止摄像机响应鼠标。务必为玩家提供清晰的方式完全退出摄像机模式，例如上面示例中的次要按钮。你也可以通过代码重新锁定光标，参见 [锁定或解锁光标](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#lock-or-unlock-the-cursor).


## 相关主题

若要读取光标在屏幕上的绝对位置，而不是其移动量，请使用 `screenCoordinates` 同一组件的属性。参见 [检查玩家的光标位置](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#check-the-players-cursor-position).

若要找出光标下方的实体，请将 `worldRayDirection` 属性与射线投射结合使用。参见 [射线投射](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting/).


> [!NOTE]
> **💡 提示**：有关指针状态的可运行示例，请查看 [`0,5-primary-cursor-info`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,5-primary-cursor-info) 测试场景，它会读取 `PrimaryPointerInfo` 每一帧并将其输入 `worldRayDirection` 到射线投射中； [`31,20-pointer-lock-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control)，它通过写入以下内容来请求并释放光标捕获： `PointerLock.isPointerLocked`；以及 [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look)，它展示了 `screenDelta` 在以下情况下仍会报告移动： `screenCoordinates` 在锁定期间被冻结在屏幕中央。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/mouse-movement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
