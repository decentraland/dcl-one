> 了解如何控制玩家的摄像机

# 摄像机

作为创作者，你可以完全控制玩家的摄像机。默认情况下，玩家在探索你的场景时可以自由选择第一人称或第三人称摄像机模式，但你也可以强制采用不同的摄像机模式。

虚拟摄像机可以是静态的，可以旋转以始终看向玩家或其他实体，也可以附着到玩家或其他实体上，从而始终伴随其移动。


> [!WARNING]
> **📔 注意**: 要在默认第一人称和第三人称摄像机之间切换，请参见 [摄像机修饰区域](#1st-and-3rd-person-camera-modes).


## 第一人称和第三人称摄像机模式

玩家通常可以通过按键盘上的 V 在第一人称和第三人称摄像机之间切换。使用一个 `CameraModeArea` 以强制该场景中位于特定区域内的所有玩家使用第一人称或第三人称摄像机模式。

```ts
const entity = engine.addEntity()

CameraModeArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	mode: CameraType.CT_FIRST_PERSON,
})
```

如果玩家当前的摄像机模式与 `CameraModeArea`不匹配，他们将切换到该摄像机模式。屏幕上会显示一个提示，说明这一变化是由于场景所致。在该区域内时，玩家无法更改其摄像机模式。当玩家离开 `CameraModeArea`时，其摄像机模式会恢复为进入前的状态。

使用 `CameraModeArea` 适用于那些玩家使用特定摄像机模式会获得明显更好体验的区域。例如，如果玩家需要点击小物体，第一人称是理想选择；或者当你的场景在玩家头顶附加了某个实体时，第三人称可能更有助于玩家注意到它。不要假设玩家知道如何切换摄像机模式，许多第一次进入的玩家可能不知道他们有这个选项，或者不记得对应的按键。


> [!WARNING]
> **📔 注意**: 摄像机修饰区域会受到其宿主实体的 Transform 组件的 *位置* 和 *旋转* 影响，但不会受到 *缩放*.



> [!WARNING]
> **📔 注意**: 如果你重叠多个摄像机修饰区域，由你的场景代码最后实例化的那个将优先于其他区域。


在创建一个 `CameraModeArea` 组件时，你必须提供以下内容：

* `区域`: 修饰区域的大小
* `模式`: 要在此区域内强制使用哪种摄像机模式，来自 `CameraType` 枚举。

支持的摄像机模式有：

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

### 查询摄像机模式

你可以使用 `CameraMode` 组件来查询玩家的摄像机模式，位于 `engine.CameraEntity`.

```ts
const cameraMode = CameraMode.get(engine.CameraEntity)
if (cameraMode.mode === CameraType.CT_FIRST_PERSON) {
	console.log('玩家正在使用第一人称摄像机')
} else {
	console.log('玩家正在使用第三人称摄像机')
}
```

你也可以使用 `onChange` 函数，位于 `CameraMode` 组件。

```ts
CameraMode.onChange(engine.CameraEntity, (cameraMode) => {
	if (!cameraMode) return
	console.log('玩家的摄像机模式已更改为', cameraMode.mode)
})
```

## 使用虚拟摄像机

要在你的场景中使用自定义摄像机行为，你需要两样东西：

* 创建一个虚拟摄像机：在你的场景中创建一个实体，并赋予它一个 `VirtualCamera`.
* 分配该虚拟摄像机：添加一个 `MainCamera` 组件到 [保留实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities) `engine.CameraEntity`，并引用带有 `VirtualCamera` 组件。

随后，摄像机将附着到带有 `VirtualCamera` 组件的实体上。如果该实体移动或旋转，摄像机也会随之移动。

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

在这个例子中，只要玩家仍在场景边界内，摄像机就会始终停留在场景中的固定位置。一旦玩家走出场景边界，默认的摄像机行为就会恢复。

### 视野

使用 `fov` 属性以度为单位为虚拟摄像机设置视野（FOV）。它仅在此虚拟摄像机处于激活状态时生效。

```ts
const cinematicCamera = engine.addEntity()
Transform.create(cinematicCamera, {
	position: Vector3.create(8, 3, 2),
})
VirtualCamera.create(cinematicCamera, {
	fov: 45,
})
```

较窄的视野（较低值）会放大画面，适合过场动画或瞄准。较宽的视野（较高值）会显示更多场景，适合全景视图。


> [!WARNING]
> **📔 注意**: 在你创建的每个虚拟摄像机上设置 `fov` 。不设置它并不会让你回退到中性的 60 度：客户端会重用池化的摄像机实例而不会重置镜头，所以没有 `fov` 的摄像机会保留之前某个摄像机留在该实例上的视野。原本用于展示宽广空中视图的摄像机可能会在一开始就处于放大状态。


你的场景可以包含任意数量带有 `VirtualCamera`组件的实体，并随着玩家移动，或随着他们执行某些操作，在多个虚拟摄像机之间动态切换。任何时刻都只有一个虚拟摄像机处于激活状态，它由 `MainCamera` 组件赋予 `engine.CameraEntity`.

要恢复为默认摄像机行为，请将值设为 `undefined` 在 `MainCamera.virtualCameraEntity`。然后玩家就可以自由切换第一人称和第三人称摄像机。如果你希望玩家只能使用这两种模式中的一种，可以使用一个 [摄像机修饰区域](#1st-and-3rd-person-camera-modes) 来强制其中一种。


> [!WARNING]
> **📔 注意**: 使用 `undefined`undefined `0`null `0` null `engine.RootEntity`已定义 `VirtualCamera`效果
> 
> 这可以捕获保存和恢复摄像机的代码，因为 `virtualCameraEntity` 是可选的，并且在新建的 `undefined` 新建的 `MainCamera`。A `?? 0` 或 `|| 0` 回退值会把它变成一个 `0` null
> 
> ```ts
> function restoreCamera(previous: Entity | undefined) {
> 	const camera = MainCamera.getMutableOrNull(engine.CameraEntity)
> 	if (!camera) return
> 	// 仅恢复真正存在的摄像机，否则释放控制权
> 	camera.virtualCameraEntity = previous && VirtualCamera.has(previous) ? previous : undefined
> }
> ```
> 



> [!WARNING]
> **📔 注意**: 只有在没有激活虚拟摄像机时，摄像机修饰区域才会对玩家产生影响。如果场景当前正在使用虚拟摄像机，而玩家走入摄像机修饰区域，什么也不会发生。
> 
> 如果一个 3D 模型在其内容中包含一个 `摄像机` 节点，则 SDK 无法使用它。你必须使用 SDK 将所有摄像机创建为实体。


```ts
function main() {
	// 自定义虚拟摄像机
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})

	// 可点击的方块
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: '重置摄像机' },
		},
		() => {
			// 将摄像机重置为默认行为
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity = undefined
		}
	)
}
```


> [!NOTE]
> **💡 提示**: 当摄像机转离头像时，通常也最好冻结头像的移动。这样玩家就不会盲目地撞上障碍物。参见 [输入修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#freeze-the-player)


要让玩家用鼠标控制虚拟摄像机，请读取 `screenDelta` 属性 `PrimaryPointerInfo` 组件的，以查看光标在每一帧移动了多远，然后将该移动应用到摄像机的旋转上。即使光标被锁定，这也有效。参见 [鼠标移动](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/mouse-movement/) 获取完整的鼠标视角示例。

## 鸟瞰视角

你可以使用虚拟摄像机从俯视角度观察场景，这可以为你的头像常规视角带来有趣的变化，并启用不同的游戏机制。

你应避免将摄像机放置在完全垂直向下的方向。相反，应始终让摄像机稍微倾斜，即使 1 度也足够。这是因为玩家方向控制是基于摄像机的视角，而不是头像的朝向。如果摄像机采用完全鸟瞰视图，那么就无法清楚地定义哪个方向代表什么。1 度的不可察觉倾斜就足以建立前进方向。

```ts
function main() {
	// 鸟瞰视角摄像机
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(8, 5, 8),
		rotation: Quaternion.fromEulerDegrees(91, 0, 0),
		// 注意旋转是 91º，而不是 90º
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## 视野

你可以使用 `fov` 属性为虚拟摄像机设置自定义视野（FOV）。其值以度为单位。务必设置它：没有可回退的默认值，而且没有 `fov` 的摄像机会继承之前使用相同池化实例的摄像机留下的镜头。

```ts
VirtualCamera.create(myCustomCamera, {
	fov: 90,
})
```

更宽的视野（较高值）会一次显示更多场景，并营造速度感，这对赛车游戏很有用。更窄的视野（较低值）会放大画面，适合瞄准或电影镜头。

### 更改缩放级别

客户端仅在摄像机变为激活状态的那一刻读取 `fov` 。对已经激活的摄像机更改 `fov` 不会产生任何作用。

要进行缩放，为每个缩放级别创建一个虚拟摄像机，并在它们之间切换 `MainCamera.virtualCameraEntity` ：

```ts
import { engine, MainCamera, Transform, VirtualCamera } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 两个摄像机位于同一位置，一个宽视角，一个放大视角
const position = Vector3.create(8, 2, 8)

const wideCamera = engine.addEntity()
Transform.create(wideCamera, { position })
VirtualCamera.create(wideCamera, { fov: 70 })

const zoomedCamera = engine.addEntity()
Transform.create(zoomedCamera, { position })
VirtualCamera.create(zoomedCamera, { fov: 30 })

// 在它们之间切换以缩放
function zoomIn() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: zoomedCamera })
}

function zoomOut() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: wideCamera })
}
```

重新分配已经激活的摄像机也不会起作用，因此每次都要切换到不同的实体。

## 摄像机过渡

每当场景在虚拟摄像机之间，或者在默认摄像机行为与虚拟摄像机之间切换时，玩家都会看到一个过渡效果。虚拟摄像机的位置、旋转以及任何其他参数都会在一段时间内平滑变化。

虚拟摄像机上的过渡设置决定了你如何过渡 *到* 该摄像机，从场景中的任何其他摄像机进入，包括默认摄像机。它们不会影响你如何过渡 *out* 离开该摄像机。

当你清除时，回到玩家自己的摄像机时 `virtualCameraEntity` 的 `MainCamera`，总是采用 0.5 秒的固定过渡，并带有缓入缓出效果。 `defaultTransition` 的虚拟摄像机不会改变这一点。

```ts
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})
```


> [!NOTE]
> **💡 提示**: 为避免出现过渡并立即切换到摄像机，请将过渡时间或速度设为 0。


根据你的使用场景，你可能更倾向于设置过渡速度而不是持续时间：

* **固定时间**: 你设置过渡的持续时间，摄像机将以完成该路径所需的速度移动，在这段时间内完成路径。
* **固定速度**: 你设置虚拟摄像机在过渡期间移动的速度，持续时间将取决于距离。用于速度的值将被解释为 **米/秒**.

以下是这两种过渡模式的示例：

```ts
// 固定持续时间
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})

// 固定速度
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Speed(3) },
})
```

以下是一个包含两个虚拟摄像机以及它们之间过渡的完整示例：

```ts
function main() {
	// 自定义虚拟摄像机 1
	const myCustomCamera1 = engine.addEntity()
	Transform.create(myCustomCamera1, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera1, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(1) },
	})

	// 自定义虚拟摄像机 2
	const myCustomCamera2 = engine.addEntity()
	Transform.create(myCustomCamera2, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera2, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(3) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera1,
	})

	// 可点击的方块
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: '重置摄像机' },
		},
		() => {
			// 将摄像机重置为默认行为
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity =
				mainCamera.virtualCameraEntity == myCustomCamera1
					? myCustomCamera2
					: myCustomCamera1
		}
	)
}
```

过渡总是沿直线移动，不会考虑路径中的任何障碍物。你也可以使用另一个虚拟摄像机作为中介，手动创建一个过渡，这样你就能完全控制它的移动。这个中介虚拟摄像机可以执行一个 [缓动](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/#move-between-two-points) ，从第一台摄像机的位置移动到第二台摄像机的位置，或者沿着一条更自定义的路径移动，避开障碍物或进行电影化绕行。

## 摄像机跟随

你可以配置一个虚拟摄像机，使其始终面向玩家的方向，或场景中的某个特定实体。摄像机的位置将保持静止，但其旋转会变化，以始终将该实体保持在画面中央。

这可以通过 `lookAtEntity` 中的 `VirtualCamera` 组件来实现。要跟随玩家，请使用 [保留实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities) `engine.PlayerEntity`.

```ts
const myCustomCamera1 = engine.addEntity()
Transform.create(myCustomCamera1, {
	position: Vector3.create(1, 2, 1),
})
VirtualCamera.create(myCustomCamera1, {
	lookAtEntity: engine.PlayerEntity,
})
```

如果摄像机正在跟随某个实体，这只会改变旋转，而不会改变摄像机的位置。

随着摄像机旋转，带有 `VirtualCamera` 组件的实体的 Transform 不会改变。不过，你可以从 `engine.CameraEntity`上的 Transform 读取摄像机的旋转。该实体的旋转和位置将是绝对的，不会受带有 `VirtualCamera` 组件的实体的影响。此变换的旋转会受到 `lookAtEntity` 行为的影响。


> [!WARNING]
> **📔 注意**: 如果你为虚拟摄像机配置一个 `lookAtEntity` ，其引用的是持有该虚拟摄像机的同一个实体，或者 `engine.CameraEntity` 实体，那么其行为将与完全不分配任何实体时相同。


## 附着到玩家

虚拟摄像机的另一种用途是通过将虚拟摄像机附着到玩家实体上，以自定义距离或角度跟随玩家。注意，玩家不能自由改变摄像机的旋转，因此在这种情况下，摄像机的旋转将固定为虚拟摄像机的旋转。例如，这对赛车游戏很有用，因为玩家通常需要始终向前看。

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(0, 1, 5),
		parent: engine.PlayerEntity,
	})
	VirtualCamera.create(myCustomCamera, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(2) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## 摄像机与碰撞体

当玩家的摄像机处于第三人称模式移动时，摄像机可能会被碰撞体阻挡，也可能不会，这取决于分配给实体的碰撞层。设计场景时要注意这一点，你可能希望防止摄像机穿过墙壁或其他实体。

参见 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#cameras-and-colliders) 以了解更多关于如何为你的场景配置碰撞体的细节。

## 旁观模式（观察者摄像机）

你可以构建一种旁观模式，将玩家从正常头像移动切换到自由漫游或跟随玩家的摄像机。这对于竞技游戏中的观察者角色、现场活动的导演摄像机或回放系统很有用。

该模式结合了多个 SDK 功能：

| 功能       | SDK API                                   | 用途                    |
| -------- | ----------------------------------------- | --------------------- |
| 自定义摄像机视图 | `VirtualCamera` + `MainCamera`            | 替换玩家的摄像机              |
| 冻结头像移动   | `InputModifier` (`disableAll: true`)      | 释放 WASD 用于控制摄像机       |
| 跟踪场景中的玩家 | `onEnterScene` / `onLeaveScene`           | 构建一个跟随目标名单            |
| 摄像机控制    | `inputSystem.isPressed(InputAction.IA_*)` | WASD 控制俯仰/偏航，E/F 控制缩放 |
| 鼠标视角     | `PrimaryPointerInfo.screenDelta`          | 使用鼠标旋转摄像机             |

### 摄像机装置架构

使用一个 **双实体装置** 以便偏航和俯仰保持独立：

```
rigRoot（实体）           -- 世界位置 + 偏航旋转
└── rigCamera（子级）      -- 俯仰旋转 + 环绕偏移
    └── VirtualCamera
```

`rigRoot` 保存偏航（左右转向），并向跟随目标或自由摄像机支点进行插值。 `rigCamera` 处理俯仰（上下倾斜）以及与根节点的环绕距离。将偏航和俯仰分离到两个 Transform 上，可以使欧拉角运算更简单。

### 启用和禁用

```ts
// 激活旁观模式
const rigRoot = engine.addEntity()
Transform.create(rigRoot, {
  position: Vector3.create(8, 8, 8),
  rotation: Quaternion.fromEulerDegrees(0, 0, 0),
})

const rigCamera = engine.addEntity()
Transform.create(rigCamera, { parent: rigRoot })
VirtualCamera.create(rigCamera, {})

MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: rigCamera })
InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: true }),
})
```

```ts
// 取消激活旁观模式
// 重要：在移除 VirtualCamera 实体之前，先清除 MainCamera。
// 如果先移除实体，引擎会继续绑定到一个已失效的实体
// 然后视角会掉到玩家的脚下。
const mainCamera = MainCamera.getMutableOrNull(engine.CameraEntity)
if (mainCamera) mainCamera.virtualCameraEntity = undefined

engine.removeEntity(rigCamera)
engine.removeEntity(rigRoot)

InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: false }),
})
```


> [!CAUTION]
> **警告：** 始终先清除 `MainCamera.virtualCameraEntity` ，然后再移除摄像机实体。先移除实体会让引擎指向一个已失效的引用，从而导致视角损坏。


### 摄像机边界

引擎会禁用 `VirtualCamera` 移出场景地块边界的实体。如果摄像机离开场景范围，它就会静默停止工作。每一帧都将摄像机位置限制在场景的轴对齐包围盒（AABB）内，并留出小余量。

```ts
// 1x1 地块场景的示例边界（16m x 16m）
const BOUNDS_MIN = Vector3.create(0, 0, 0)
const BOUNDS_MAX = Vector3.create(16, 20, 16)
const BOUNDS_MARGIN = 0.5
```

对于更大的场景，请将边界设置为与你的 `scene.json` 地块相匹配。无论地块数量多少，场景高度上限都是 330 米。一个 4x4 地块的场景可以使用 `Vector3.create(64, 330, 64)`.

### 跟随玩家

使用 `onEnterScene` 和 `onLeaveScene` 以构建场景中玩家的实时名单。然后玩家可以使用按键在跟随目标之间轮换（例如， `IA_ACTION_3` 和 `IA_ACTION_4`）。当跟随某个玩家时，装置根节点会朝被跟随玩家的 Transform 位置做插值，而子摄像机则以可配置的距离绕行。

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

const playerEntities = new Map<string, Entity>()
let playerIds: string[] = []

onEnterScene((player) => {
  if (!player) return
  playerIds.push(player.userId)
  playerEntities.set(player.userId, player.entity)
})

onLeaveScene((userId) => {
  if (!userId) return
  playerIds = playerIds.filter((id) => id !== userId)
  playerEntities.delete(userId)
})
```

要在每一帧跟随玩家，请让装置根节点向其位置做插值：

```ts
const followEntity = playerEntities.get(followTargetId)
if (followEntity) {
  const targetPos = Transform.get(followEntity).position
  const rootTransform = Transform.getMutable(rigRoot)
  rootTransform.position = Vector3.lerp(
    rootTransform.position,
    Vector3.add(targetPos, Vector3.create(0, 1, 0)),
    0.1
  )
}
```

参见 [玩家进入或离开场景](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-enters-or-leaves-scene) 更多内容，请参见 `onEnterScene` 和 `onLeaveScene`.

### 观战时使用鼠标视角

阅读 `PrimaryPointerInfo.screenDelta` 每一帧都用鼠标旋转相机。此功能在指针被锁定时有效。参见 [鼠标移动](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/mouse-movement/) 获取完整的鼠标视角示例。

```ts
const MOUSE_SENSITIVITY = 0.15 // 每像素角度
let yaw = 0
let pitch = 45

function spectateMouseLook() {
  const isLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  if (!isLocked) return

  const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
  if (!delta) return

  yaw = (yaw + delta.x * MOUSE_SENSITIVITY) % 360
  // delta.y 向下增大，因此鼠标向上为负值：将其相加会让相机向上倾斜；使用限制以防翻转
  pitch = Math.max(-25, Math.min(80, pitch + delta.y * MOUSE_SENSITIVITY))
}
```


> [!WARNING]
> **注意：** `screenDelta` 仅限桌面端。在移动端它始终报告 `0`。如果你的场景面向移动端，请设计基于触控的备用方案。


### 完整参考实现

该 [`33,20-观战模式`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) 测试场景包含一个自包含的观战模块，位于 `src/spectate.ts`。它支持自由相机和跟随相机模式、WASD + 鼠标视角控制、玩家切换、轨道距离缩放、边界限制，以及一个屏幕 HUD，显示控制说明和当前跟随目标。

要在你自己的项目中使用它：

1. 复制 `src/spectate.ts` 到你的场景中。
2. 更新 `PIVOT`, `BOUNDS_MIN`，以及 `BOUNDS_MAX` 在文件顶部，使其与你的场景地块匹配。这是最常见的集成错误。
3. 连接 `toggleSpectate()` 到任意触发器：可点击实体、UI 按钮或按键。


> [!NOTE]
> **💡 提示**：有关相机控制的可用示例，请查看 [`2,22-虚拟相机`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) 测试场景，其中轮换多个 `VirtualCamera` 实体经过 `MainCamera` 包括 `lookAtEntity` 朝向玩家的相机以及一个补间相机； [`32,20-虚拟相机鼠标视角`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look)，它在指针锁定时驱动一个鼠标视角相机； [`33,20-观战模式`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode)，它实现了完整的观战/观察者相机，具有跟随相机和自由相机模式；以及 [`9,99-修饰区域`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas)，它会在一个体积区域内强制一种相机模式，并带有 `CameraModeArea`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
