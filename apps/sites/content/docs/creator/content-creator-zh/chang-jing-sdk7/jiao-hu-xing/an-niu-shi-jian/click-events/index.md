> 了解如何处理场景中的用户点击。

# 点击事件

Decentraland 场景可以检测用于控制玩家头像的所有按钮输入操作。这些按钮包括指针点击、若干动作按钮，以及用于移动头像的按键。按钮事件可以来自桌面客户端的鼠标和键盘，来自屏幕上的控制， [移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)来自 VR 控制器，或来自其他输入设备——SDK 会将它们一视同仁地解释。

你可以检测针对某个实体的输入操作。这需要在玩家的光标指向该实体的碰撞器时按下按钮。你也可以检测 *全局* 输入事件，它涉及在任何时候按下并激活输入，而不考虑指针指向哪里。


> [!WARNING]
> **📔 注意**：实体必须有一个 [碰撞器](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/) 才能响应输入操作。 `MeshRenderer` 模型还必须配备一个 `MeshCollider` 组件。来自 `GltfContainer` 的模型可能有自己内嵌的碰撞几何体，或者可以配置为使用其可见几何体，它们也可以被赋予一个 `MeshCollider` 组件。


根据用例，有几种不同的方式来处理输入操作。

* [**注册回调**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/)：为单个实体添加交互的最简单方式。编写一条语句来设置回调函数和悬停反馈。
* [**基于系统**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/)：适合处理行为相似的多个实体。使用系统遍历相似实体并分别查询每个实体的输入操作，以相同逻辑处理它们。悬停反馈需要单独设置。这种方法也用于处理全局输入操作。
* [**高级**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/advanced-button-events/)：读取每个实体上的原始响应数据，包括时间戳和输入事件历史记录。这对于定义自定义交互模式很有用。

## 在 Creator Hub 中使用场景编辑器

处理实体点击事件的最简单方式是使用场景编辑器。使用无代码的 **点击时** 或 **输入操作时** 在物品上触发，以便在点击它时调用动作。或者使用 **全局点击时**, **全局主键时** 或 **全局次键时** 触发器来响应全局按钮事件。参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).

## 简单示例

要检测对实体的点击，请使用 `pointerEventsSystem.onPointerDown`.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		console.log('clicked entity')
	}
)
```

参见 [**注册回调**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 了解更多信息。

## 悬停反馈

让玩家知道某个实体是可交互的很重要。否则，他们可能会完全错过你构建的体验。逐个点击每个对象并希望其中一个有反应，并不是好的体验。

当你使用 [**注册回调**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 方法时，每当玩家将光标移到对象上方时，会显示两种反馈：

* 实体边缘会被高亮（仅在 Decentraland 2.0 桌面客户端中）。如果实体距离足够近可以点击，高亮为绿色；如果实体太远，则为红色。
* 光标附近会出现带有 UI 文本的悬停提示，指示如果点击会发生什么。

当使用 [**基于系统**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 方法时，你可以通过添加一个 `PointerEvents` 组件到可点击实体上来实现相同效果。

实体高亮和悬停提示都可以通过这些方法和组件中的属性禁用。

边缘高亮会勾勒出光标所在碰撞器对应的实体，以及该实体的所有子级。它不会勾勒该实体的父级或其兄弟级。只有来自 `MeshRenderer` 以及来自 `GltfContainer` 的模型才会获得轮廓高亮， `TextShape` 文本不会。如果你使用一个不可见的盒子作为更大的可点击区域，请将可见部分设为该盒子实体的子级，否则不会高亮任何东西。

你也可以实现 [高级自定义提示](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/#advanced-custom-hints)，例如播放声音，让实体在被指向时改变颜色、旋转或放大等。无论你怎么做，都要确保这是一个清晰的提示标识。

## 障碍

按钮事件会发射射线，只与路径上订阅了指针事件碰撞层的第一个实体交互。只要实体在其距离限制内，这一点就成立。如果实体同时为摄像机和头像定义了距离限制，那么两者中任意一个足够近即可。

请在具有碰撞器的同一实体上注册指针事件。注册在父实体上，或注册在没有自身碰撞器的子实体上的指针事件都不会触发。

要使实体被指针事件的射线拦截，必须满足以下任一条件：

* 模型必须包含 [碰撞网格](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/).
* 该 `GltfContainer` 必须配置为使用 [带碰撞遮罩的可见几何体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#colliders-on-3d-models).
* 实体必须有一个 [MeshCollider 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).

如果另一个实体的碰撞器挡在玩家想要交互的实体前面，玩家将无法点击后面的实体，除非该实体没有碰撞器，或者此碰撞器被配置为不响应指针事件碰撞层。

```ts
// 可点击实体
const clickableEntity = engine.addEntity()
MeshRenderer.setBox(clickableEntity)
MeshCollider.setBox(clickableEntity)
Transform.create(clickableEntity, { position: Vector3.create(8, 1, 8) })

pointerEventsSystem.onPointerDown(
	{
		entity: clickableEntity,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Click',
		},
	},
	function () {
		console.log('clicked entity')
		const t = Transform.getMutable(clickableEntity)
		t.scale.y += 0.2
	}
)

// 不阻挡点击
const nonBlocker = engine.addEntity()
MeshRenderer.setBox(nonBlocker)
MeshCollider.setBox(nonBlocker, ColliderLayer.CL_PHYSICS)
Transform.create(nonBlocker, { position: Vector3.create(10, 1, 8) })

// 点击阻挡器
const blocker = engine.addEntity()
MeshRenderer.setBox(blocker)
MeshCollider.setBox(blocker, ColliderLayer.CL_POINTER)
Transform.create(blocker, { position: Vector3.create(8, 1, 10) })
```


> [!WARNING]
> **📔 注意**：要让实体不仅拦截指针事件，还能返回数据，该实体还需要有一个 `PointerEvents` 组件。 `pointerEventsSystem` 辅助函数也会处理这个需求。


## 指针按钮

以下输入可由任一检测输入事件的方法处理。

* `InputAction.IA_POINTER`: **鼠标左键** 在电脑上， **交互按钮** 在移动端。
* `InputAction.IA_PRIMARY`: **E** 在电脑上，按键， **E 按钮** 在移动端。
* `InputAction.IA_SECONDARY`: **F** 在电脑上，按键， **F 按钮** 在移动端。
* `InputAction.IA_ACTION_3`: **1** 在电脑上，按键， **1 键** 在移动端。 *在移动端默认隐藏（SDK 7.32.0+）——仅当你的场景通过 `TouchScreenControls`.*
* `InputAction.IA_ACTION_4`: **2** 在电脑上，按键， **2 键** 在移动端。 *在移动端默认隐藏（SDK 7.32.0+）——仅当你的场景通过 `TouchScreenControls`.*
* `InputAction.IA_ACTION_5`: **3** 在电脑上，按键， **3 键** 在移动端。 *在移动端默认隐藏（SDK 7.32.0+）——仅当你的场景通过 `TouchScreenControls`.*
* `InputAction.IA_ACTION_6`: **4** 在电脑上，按键， **4 键** 在移动端。 *在移动端默认隐藏（SDK 7.32.0+）——仅当你的场景通过 `TouchScreenControls`.*
* `InputAction.IA_JUMP`: **空格** 在电脑上，按键， **跳跃按钮** 在移动端。
* `InputAction.IA_FORWARD`: **W** 在电脑上按键。
* `InputAction.IA_LEFT`: **A** 在电脑上按键。
* `InputAction.IA_RIGHT`: **D** 在电脑上按键。
* `InputAction.IA_BACKWARD`: **S** 在电脑上按键。
* `InputAction.IA_WALK`: **Control** 在电脑上按键。
* `InputAction.IA_MODIFIER`: **Shift** 在电脑上按键。

每个 `InputAction` 与键盘上的具体输入进行了抽象隔离，因此可以根据设备映射到不同输入。出于同样的原因，并非键盘上的所有按键都能被跟踪为按钮事件，只有用于移动和交互的按键可以。这个有意的限制确保内容能在桌面端、 [移动客户端](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)以及未来的设备（如 VR 控制器和其他游戏手柄）上保持兼容。


> [!WARNING]
> **📱 移动端**：在 [移动客户端](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)上，每个输入操作都可以路由到屏幕按钮，但默认只显示三个按钮： `IA_POINTER` （交互按钮）， `IA_PRIMARY` （E 按钮）和 `IA_JUMP`. `IA_SECONDARY` （F）和 `IA_ACTION_3`–`IA_ACTION_6` （即 `1`/`2`/`3`/`4` 按钮）默认起始为隐藏状态（SDK 7.32.0 及以后；较旧的 SDK 会显示全部 8 个，较低的按钮会被“+”溢出菜单遮住）。在为移动端设计时，优先使用 `IA_POINTER`, `IA_PRIMARY` 和 `IA_JUMP` 用于你的关键操作。如果你需要 F 或某个数字按钮，请使用 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/)。请参见 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/).


## 指针事件类型

每个输入都可以产生以下类型的指针事件。下面每一项都是 `PointerEventType` 枚举中的一个值。

* `PET_DOWN`：玩家在光标指向实体的碰撞器时按下某个特定按钮。
* `PET_UP`：玩家在光标指向实体的碰撞器时松开某个特定按钮。
* `PET_HOVER_ENTER`：玩家的光标开始指向实体的碰撞器。
* `PET_HOVER_LEAVE`：玩家的光标停止指向实体的碰撞器。
* `PET_PROXIMITY_ENTER`：玩家进入实体的接近范围，无论他们看向哪里。
* `PET_PROXIMITY_LEAVE`：玩家离开实体的接近范围。

参见 [**接近事件**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/) 了解如何使用基于接近的交互。

## 输入操作的数据

所有输入操作都包含有关事件的数据，包括被激活的按钮以及当时指针指向的位置等。

任何输入事件都可以获取以下信息：

* `analog`：可选数字，仅在来自模拟输入（如摇杆）的事件中存在，用于存储输入的模拟值。
* `button`：按下的是哪个按钮 ID。该数字对应于 `InputAction` 枚举，其中列出了所有可用按钮。
* `state`：指针事件类型，来自枚举 `PointerEventType`. *0* 指的是 `PointerEventType.PET_UP`, *1* 设置为 `PointerEventType.PET_DOWN`, *2* 设置为 `PointerEventType.PET_HOVER_ENTER`, *3* 设置为 `PointerEventType.PET_HOVER_LEAVE`, *4* 设置为 `PointerEventType.PET_PROXIMITY_ENTER`, *5* 设置为 `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`：一个 [Lamport 时间戳](https://en.wikipedia.org/wiki/Lamport_timestamp) 用于标识每个按钮事件。

  > 注意：此时间戳并不是基于当前时间编号的。可以把它看作一个从 0 开始、每个事件递增 1 的计数器。
* `命中`：一个包含以下命中事件数据的对象：
  * `entityId`：被射线命中的实体 ID 号。
  * `meshName`: *字符串* ，包含被命中的 3D 模型中具体网格的内部名称。当一个 3D 模型由多个网格组成时，这很有用。
  * `globalOrigin`: *Vector3* 用于表示射线起点的位置（相对于场景）
  * `direction`: *Vector3* ，包含射线的方向向量，以全局坐标表示
  * `位置`: *Vector3* 用于表示射线与命中实体相交的位置（相对于场景）
  * `length`：射线从起点到与实体发生命中的位置之间的长度。
  * `normalHit`: *Vector3* ，包含一个归一化方向向量，用于描述世界空间中命中法线的角度。

根据你处理输入操作所采用的方法，此数据的访问方式不同。

使用 [**注册回调**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 方法时，传递给回调函数的第一个参数包含整个数据结构。

```ts
pointerEventsSystem.onPointerDown({ entity: myEntity }, function (cmd) {
	console.log(cmd.hit?.entityId)
})
```

使用 [**基于系统**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 方法时，请使用 `inputSystem.getInputCommand()` 来获取此数据。

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```


> [!WARNING]
> **📔 注意**：要让实体不仅拦截指针事件，还能返回数据，该实体还需要有一个 `PointerEvents` 组件。 `pointerEventsSystem` 辅助函数也会处理这个需求。


使用 [**高级**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/advanced-button-events/) 方法中， `PointerEventsResult` 包含一个列表，记录该实体最近的所有指针事件历史。

```ts
engine.addSystem(() => {
	for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
		const pointerEvents = PointerEventsResult.get(entity)

		for (const event of pointerEvents) {
			console.log(event.hit?.entityId)
		}
	}
})
```

## 锁定或解锁光标


> [!NOTE]
> **💡 注**: `PointerLock` 是桌面客户端中的概念（鼠标光标锁定与解锁）。它对 [移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)没有影响，在那里输入基于触摸。


Decentraland 桌面客户端中的玩家可以在两种不同的光标交互模式之间切换：

* 锁定光标：移动鼠标会转动摄像机，点击事件发生在屏幕中央的准星处
* 解锁光标：光标可以独立于摄像机移动，玩家可以点击屏幕任意位置，与 3D 空间或 UI 交互

这些交互模式适合不同类型的游戏机制，你可能希望让玩家根据最适合你场景的方式采用其中一种。你也可能希望临时确保玩家的光标处于解锁状态，以便他们与 UI 交互，这对不熟悉该控制方式的新玩家尤其有帮助。

要更改玩家的光标状态，请使用 `PointerLock` 组件在 `engine.CameraEntity` 实体上。

```ts
import {PointerLock} from '@dcl/sdk/ecs'
    
PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false})
```


> [!WARNING]
> **📔 注意**：引擎提供一个 `PointerLock` 组件在 `engine.CameraEntity` 默认情况下。使用 `createOrReplace()` 或 `getMutable()` 而不是 `create()`，如果组件已存在则会抛出错误。


你也可以通过读取 `PointerLock` 组件的状态来查询玩家的光标状态。

```ts
import {PointerLock} from '@dcl/sdk/ecs'

const isPointerLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked
```

使用 `.onChange` 函数来响应光标状态变化。下面的示例强制光标始终处于解锁状态。

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


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
