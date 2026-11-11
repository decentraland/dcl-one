> 如何随时间逐步移动、旋转和缩放实体，并以增量方式变化。

# 移动实体

若要在一段时间内移动、旋转或缩放场景中的实体，请使用 `Tween` 组件。引擎会平滑地执行所需的变换，在每一帧显示更新，直到指定时长结束。此外， `Transform` 受影响实体的组件值会实时更新，以便在场景代码中需要进行邻近检查时使用。


> [!NOTE]
> **💡 提示**：在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)中，你可以通过 **操作**，参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).


Tween 组件具有以下功能：

* `setMove`：在两个点之间移动
* `setRotate`：在两个方向之间旋转
* `setScale`：在两个大小之间缩放
* `setMoveRotateScale`：同时在这三个参数上过渡
* `setMoveContinuous`：沿相同方向持续移动
* `setRotateContinuous`：沿相同方向持续旋转
* `setTextureMove`：在两个位置之间偏移材质的纹理
* `setTextureMoveContinuous`：沿相同方向持续偏移材质的纹理

## 在两个点之间移动

若要让实体在两个点之间移动，请创建一个 `Tween` 组件，并使用 `setMove` 函数。

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)
```

移动补间需要以下信息：

* `entity`：要移动的实体
* `start`：表示起始位置的 Vector3
* `end`：表示结束位置的 Vector3
* `duration`：在两个位置之间移动需要多少毫秒

此外，还有一个可选参数：

* `easingFunction`：要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens)

## 在两个方向之间旋转

若要让实体在两个点之间旋转，请创建一个 `Tween` 组件，并使用 `setRotate` 函数。

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setRotate(myEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

旋转补间需要以下信息：

* `start`：表示起始旋转的 Quaternion
* `end`：表示结束旋转的 Quaternion
* `duration`：在两个位置之间移动需要多少毫秒

此外，还有一个可选参数：

* `easingFunction`：要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens)

### 围绕枢轴点旋转

在旋转实体时，旋转始终是相对于实体中心坐标进行的。若要使用另一组坐标作为枢轴点来旋转实体，请创建第二个（不可见的）实体，将枢轴点设为其位置，并让它成为你想要旋转的实体的父级。

当旋转父实体时，其子实体也会全部围绕父实体的位置作为枢轴点进行旋转。请注意， `位置` 子实体的是相对于父实体的位置。

```ts
const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)

Tween.setRotate(pivotEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

请注意，在此示例中，系统正在旋转 `pivotEntity` 实体，它是 `childEntity` 实体的父级。

## 在两个大小之间缩放

若要将实体的缩放在两个大小之间变化，请创建一个 `Tween` 组件，并使用 `setScale` 函数。

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)	

```

缩放补间需要以下信息：

* `start`：表示起始大小的 Vector3
* `end`：表示结束大小的 Vector3
* `duration`：在两个位置之间移动需要多少毫秒

此外，还有一个可选参数：

* `easingFunction`：要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens)

## 从当前位置移动

当补间处于活动状态时，引擎会实时更新实体的 `Transform` 组件值。这意味着你可以在任何时刻从实体的 `Transform` 组件中读取实体当前的位置，并将该值作为新补间的 `start` 。然后实体会从它当前所在的位置移动到目标位置，无论它是如何到达那里的。

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '移动' },
	},
	() => {
		Tween.setMove(myEntity, 
			Transform.get(myEntity).position, 
			Vector3.create(8, 1, 8), 
			2000,
			EasingFunction.EF_EASEOUTQUAD
		)
	}
)
```


> [!WARNING]
> **📔 注**：始终传入明确的 `start` 值。如果 `start` 参数未设置，它会被视为 *(0, 0, 0)*，因此实体会在移动前传送到场景原点。同样地，如果你传入一个与实体当前位置不匹配的硬编码 `start` ，实体会在移动前明显传送到该位置。只要你希望移动从实体当前所在位置开始，请像本节示例中那样，从实体的 `Transform` 组件中读取当前位置。


### 在移动过程中重新设定目标

调用 `setMove` （或其他任何补间函数）到一个已经在运行补间的实体上，会用新的补间替换正在运行的补间。由于 `start` 是从实体的实时位置读取的，因此这会在移动途中平滑地重新定向实体：它会从当前时刻所在的位置改变路线，不会出现跳跃。

下面的示例会把实体移动到玩家点击的任意平台。如果玩家在实体仍在移动时点击另一个平台，实体会平滑地转向新的目标。

```ts
function moveTo(target: Vector3) {
	Tween.setMove(shipEntity, 
		Transform.get(shipEntity).position, 
		target, 
		2000,
		EasingFunction.EF_EASEOUTQUAD
	)
}

pointerEventsSystem.onPointerDown(
	{
		entity: padA,
		opts: { button: InputAction.IA_POINTER, hoverText: '移到这里' },
	},
	() => moveTo(Vector3.create(2, 1, 2))
)

pointerEventsSystem.onPointerDown(
	{
		entity: padB,
		opts: { button: InputAction.IA_POINTER, hoverText: '移到这里' },
	},
	() => moveTo(Vector3.create(14, 1, 14))
)
```

若要在实体到达目标时运行逻辑，请参见 [补间结束时](#on-tween-finished).


> [!WARNING]
> **📔 注**：仅在响应偶发事件时以这种方式重新设定目标，例如点击或到达航点。如果目标持续变化，例如一个跟随玩家的实体，不要每一帧都创建新的补间：实体会明显卡顿。请参见 [跟随移动目标](#follow-a-moving-target).


## 非线性补间

补间可以遵循不同的 **缓动函数** ，它们会影响变化率随时间的变化。一个 **线性** 函数表示变化的速度从开始到结束保持恒定。有很多选项可供选择，它们会根据开始和/或结束时是缓慢起步，以及缓慢到什么程度，绘制出不同形状的曲线。一个 **easeinexpo** 曲线开始较慢，结束较快，速度呈指数增长；相反， **easeoutexpo** 曲线则是开始较快，结束较慢。


> [!NOTE]
> **💡 提示**：尝试不同的运动曲线。差异往往很细微，但我们会无意识地通过事物的运动方式来解读信息，比如重量、摩擦力，甚至性格。


```ts
Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000,
	EasingFunction.EF_EASEOUTBOUNCE
)

```

可选的 `easingFunction` 参数取值自 `EasingFunction` 枚举，该枚举提供以下选项：

* `EF_EASEBACK`
* `EF_EASEBOUNCE`
* `EF_EASECIRC`
* `EF_EASECUBIC`
* `EF_EASEELASTIC`
* `EF_EASEEXPO`
* `EF_EASEINBACK`
* `EF_EASEINBOUNCE`
* `EF_EASEINCIRC`
* `EF_EASEINCUBIC`
* `EF_EASEINELASTIC`
* `EF_EASEINEXPO`
* `EF_EASEINQUAD`
* `EF_EASEINQUART`
* `EF_EASEINQUINT`
* `EF_EASEINSINE`
* `EF_EASEOUTBACK`
* `EF_EASEOUTBOUNCE`
* `EF_EASEOUTCIRC`
* `EF_EASEOUTCUBIC`
* `EF_EASEOUTELASTIC`
* `EF_EASEOUTEXPO`
* `EF_EASEOUTQUAD`
* `EF_EASEOUTQUART`
* `EF_EASEOUTQUINT`
* `EF_EASEOUTSINE`
* `EF_EASEQUAD`
* `EF_EASEQUART`
* `EF_EASEQUINT`
* `EF_EASESINE`
* `EF_LINEAR`

## 持续旋转

若要让实体持续旋转，请使用 `Tween` 组件，并使用 `setRotateContinuous` 函数。

```ts
Tween.setRotateContinuous(myEntity, 
	Quaternion.fromEulerDegrees(0, -1, 0), 
	45
)
```

持续旋转补间需要以下信息：

* `entity`：要旋转的实体
* `direction`：一个用于决定绕哪条轴旋转的 Quaternion。只有该旋转的轴才有意义，角度大小会被忽略。例如， `Quaternion.fromEulerDegrees(0, -1, 0)` 和 `Quaternion.fromEulerDegrees(0, -90, 0)` 的行为相同，都是围绕 *y* 轴朝负方向旋转。
* `speed`：实体每秒将旋转多少度。负值会向相反方向旋转。

此外，还有一个可选参数：

* `duration`：维持旋转多少毫秒。到达此时间后，旋转会停止。

## 持续移动

若要让实体沿相同方向持续移动，请使用 `Tween` 组件，并使用 `setMoveContinuous` 函数。

```ts
Tween.setMoveContinuous(myEntity, 
	Vector3.create(0, 0, 1), 
	0.7
)
```

持续移动补间需要以下信息：

* `entity`：要移动的实体
* `direction`：表示移动方向的 Vector3
* `speed`：实体每秒将移动多少米

此外，还有一个可选参数：

* `duration`：维持移动多少毫秒。到达此时间后，移动会停止。

持续移动补间需要以下信息：

### 跟随移动目标

`setMoveContinuous` 当目标持续变化时，这也是正确的工具，例如一个跟随玩家的实体。你可能会想改为每一帧都创建一个新的 `setMove` 补间，指向玩家的最新位置，但这会产生明显的卡顿。

原因在于，你从 `Transform` 组件中读取到的补间实体位置，是引擎上一次回传给场景的位置，因此它会比实体真正所在的位置滞后几帧。当你将该值作为新补间的 `start` 时，引擎会立即应用它，实体就会跳回到几帧之前所在的位置。这样的修正一次几乎察觉不到，但每秒重复很多次就会很明显。

`setMoveContinuous` 完全避免了这个问题，因为你提供的是方向和速度，而不是起始点。引擎会持续使用实体的真实位置，因此在更改方向时替换补间也不会让实体跳跃。

```ts
const CHASE_SPEED = 3
const STOP_DISTANCE = 1

engine.addSystem(() => {
	const playerPosition = Transform.get(engine.PlayerEntity).position
	const myPosition = Transform.get(myEntity).position

	if (Vector3.distance(myPosition, playerPosition) <= STOP_DISTANCE) {
		// 足够近了：持续补间没有目标点，所以需要你自己停止它
		if (Tween.has(myEntity)) {
			Tween.deleteFrom(myEntity)
		}
		return
	}

	const direction = Vector3.subtract(playerPosition, myPosition)
	direction.y = 0 // 即使玩家跳跃，也保持在地面上

	Tween.setMoveContinuous(myEntity, Vector3.normalize(direction), CHASE_SPEED)
})
```


> [!WARNING]
> **📔 注**：持续补间不会自行结束，因此实体会一直移动，直到你移除 `Tween` 组件，使用 `Tween.deleteFrom()`，或者直到可选的 `duration` 耗尽为止。另请注意，每一帧重新设定目标都会每一帧向引擎发送一次更新。为避免这种情况，只有在方向变化到足以产生影响时才再次调用 `setMoveContinuous` 。


## 补间序列

若要让实体按顺序播放一系列补间，请使用 `TweenSequence` 组件。该组件需要两个字段：

* `sequence`：包含多个补间定义的数组，这些补间将按顺序执行。数组可以为空，在这种情况下，它只会播放当前补间。
* `loop` *（可选）*：如果未提供，则序列只播放一次。如果提供了该字段，其值必须是 `TweenLoop` 枚举的值。可接受的值有：
  * `TL_RESTART`：当序列结束时，它会重新开始。如果最后状态与第一个状态不匹配，实体会立即从一个状态跳到另一个状态。
  * `TL_YOYO`：当序列结束时，它会反向执行，按相反顺序播放所有补间，直到再次回到起点。然后它会再次开始。

### 来回移动

若要让平台在两个位置之间持续来回移动，请将 `sequence` 数组留空，并设置 `loop` 设为 `TweenLoop.TL_YOYO`

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)

TweenSequence.create(myEntity, { sequence: [], loop: TweenLoop.TL_YOYO })
```

实体会在起点和终点之间来回移动，两个方向的持续时间和缓动函数都相同。

### 沿路径移动

若要让实体沿包含多个点的更复杂路径移动，请在 `sequence` 中提供一个补间定义列表，作为一个 `TweenSequence` 组件，使其在场景运行时可用。

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(6.5, 7, 4), 
	Vector3.create(6.5, 7, 12), 
	4000
)

TweenSequence.create(myEntity, {
	sequence: [
		{
			duration: 2000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 7, 12),
				end: Vector3.create(6.5, 10.5, 12),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 12),
				end: Vector3.create(6.5, 10.5, 4),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 4),
				end: Vector3.create(6.5, 7, 4),
			}),
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

请注意，在 TweenSequence 中定义补间时，你需要使用更详细的 `Tween.Mode.Move`，或 `Tween.Mode.Rotate`，或 `Tween.Mode.Scale` 格式来定义补间。在这种更详细的格式中，你需要指定：

* `duration`：在两个位置之间移动需要多少毫秒
* `easingFunction`：要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens)。在此格式中，该值是必需的。
* `mode`：补间的模式，可以是 `Tween.Mode.Move`, `Tween.Mode.Rotate`，或 `Tween.Mode.Scale`.

并且在 `mode` 字段中，你需要指定：

* `start`：补间的起始值
* `end`：补间的结束值

## 补间结束时

使用 `tweenSystem.tweenCompleted` 来检测补间何时结束。这在补间结束时执行操作时很有用，例如打开电梯门。

```ts
engine.addSystem(() => {
	const tweenCompleted = tweenSystem.tweenCompleted(myEntity)
	if (tweenCompleted) {
		//播放声音
	}
})
```

## 同时补间

若要让实体在初始状态和结束状态之间移动、旋转并缩放，请创建一个 `Tween` 组件，并使用 `setMoveRotateScale` 函数。该函数也可用于这三个参数的任意组合。

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(14, 1, 2),
})
MeshRenderer.setBox(myEntity)

Tween.setMoveRotateScale(myEntity, {
	position: { start: Vector3.create(14, 1, 2), end: Vector3.create(14, 3, 2) },
	rotation: { start: Quaternion.fromEulerDegrees(0, 0, 0), end: Quaternion.fromEulerDegrees(0, 180, 90) },
	scale: { start: Vector3.One(), end: Vector3.create(2, 0.5, 2) },
	duration: 2000
})
```

移动补间需要以下信息：

* `entity`：要移动的实体
* `params`：一个包含以下参数的对象
  * `位置`: *（可选）* 一个具有 `start` 和 `end` 值的对象，既作为 `Vector3`.
  * `rotation`: *（可选）* 一个具有 `start` 和 `end` 值的对象，既作为 `Quaternion`.
  * `scale`: *（可选）* 一个具有 `start` 和 `end` 值的对象，既作为 `Vector3`。至少必须提供 `位置`, `rotation` 或 `scale` 之一。
  * `duration`: *（必需）* 在两组值之间过渡需要多少毫秒
  * `easingFunction`: *（可选）* 要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens)

一个实体只能有一个 `Tween` 组件，并且每个补间组件一次只能执行一种变换。通过 `setMoveRotateScale` 补间类型，你可以让实体横向移动并同时旋转，但这两种运动会遵循同一时间线。如果你需要彼此独立的过渡，作为变通方法，可以使用具有父子关系的实体。例如，你可以让一个不可见的父实体横向移动，让一个可见的子实体旋转。

在下面的代码片段中，父实体在旋转，而子实体在放大。

```ts
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(parentEntity)
Tween.setRotate(parentEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	5000
)

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(0, 0, 0),
	parent: parentEntity,
})
MeshRenderer.setBox(childEntity)
Tween.setScale(childEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)
```

## 暂停补间

若要暂停补间，请将 `playing` 属性改为 false。若要恢复，请将其改回 true。

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: '暂停' },
	},
	() => {
		let tweenData = Tween.getMutable(myEntity)
		tweenData.playing = !tweenData.playing
	}
)
```

若要结束一个不需要继续的补间，请删除实体上的 `Tween` 组件。如果该实体还在使用 `TweenSequence` 组件，也一并删除它。

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: '暂停' },
	},
	() => {
		if( Tween.has(myEntity)){
			Tween.deleteFrom(myEntity)
		}
		if( TweenSequence.has(myEntity)){
			TweenSequence.deleteFrom(myEntity)
		}
	}
)
```

## 基于系统的 Tween

而不是使用 Tween 组件并让引擎处理变换，你可能更愿意通过一个 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 在你的场景中。每次该函数运行时，都会让实体移动一点点。

一方面，这让你在每一帧重新计算运动时拥有更多控制权。另一方面，代码更复杂，而且性能较差的机器上的玩家可能会觉得 tween 有卡顿感，能注意到每次增量。

### 通过系统移动

移动实体最简单的方法是逐步修改 *位置* 存储在 `Transform` 组件，使其在场景运行时可用。

```ts
function SimpleMove() {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), 0.05)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

在这个例子中，我们让实体在游戏循环的每个 tick 中移动 0.05 米。

`Vector3.Forward()` 返回一个朝前的向量，长度为 1 米。在这个例子中，我们再通过 `Vector3.scale()`将这个向量缩小到其长度的 1/20。如果我们的场景每秒 30 帧，该实体的移动速度就是每秒 1.5 米。

![](/docs/_remote/90ee9b2fdf4c6fb2a573ce79.webp)

### 通过系统旋转

旋转实体最简单的方法是逐步改变 Transform 组件中的值，并将其作为系统函数的一部分来运行。

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

注意，为了将当前旋转与每个增量合并，我们使用 `Quaternion.multiply`。在四元数运算中，你是通过相乘来组合两个旋转的，而不是相加。将一个四元数与另一个相乘得到的结果旋转，等价于先执行一个旋转再执行另一个旋转后的最终旋转。

在这个例子中，我们让实体在游戏循环的每个 tick 中沿向上方向旋转 1 度。


> [!NOTE]
> **💡 提示**：要让实体始终旋转并面向玩家，你可以添加一个 [`Billboard` 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#face-the-player).


![](/docs/_remote/13d2f2aa20f3bb7131ff9274.webp)

### 通过系统围绕枢轴点旋转

在旋转实体时，旋转始终是相对于实体中心坐标进行的。若要使用另一组坐标作为枢轴点来旋转实体，请创建第二个（不可见的）实体，将枢轴点设为其位置，并让它成为你想要旋转的实体的父级。

当旋转父实体时，其子实体也会全部围绕父实体的位置作为枢轴点进行旋转。请注意， `位置` 子实体的是相对于父实体的位置。

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(pivotEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)
```

请注意，在此示例中，系统正在旋转 `pivotEntity` 实体，它是 `childEntity` 实体的父级。

![](/docs/_remote/3c145695aa57e42efe942e39.webp)

### 调整运动以适应延迟时间

假设访问你场景的玩家难以跟上帧率节奏。这可能会导致运动看起来跳跃，因为并不是所有帧的时间间隔都相同，但每一帧都会让实体移动相同的量。

你可以通过使用 `dt` 参数来调整运动的缩放。

```ts
function SimpleMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), dt)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

上面的示例即使在帧率下降时，也会让运动速度大致与上面的运动示例保持一致。当以每秒 30 帧运行时， `dt` 的值是 1/30。

你也可以用同样的方法平滑旋转，只需将旋转量乘以 `dt`.

### 通过系统在两个点之间移动

如果你希望实体平滑地在两个点之间移动，请使用 *lerp* （线性插值）算法。这个算法在游戏开发中非常有名，因为它非常有用。

该 `lerp()` 函数接受三个参数：

* 起始位置向量
* 目标位置向量
* 数量，一个从 0 到 1 的值，表示要执行多少比例的平移。

```ts
const originVector = Vector3.Zero()
const targetVector = Vector3.Forward()

let newPos = Vector3.lerp(originVector, targetVector, 0.6)
```

线性插值算法会在两者向量之间的路径上找到一个中间点，该点与所提供的数量相匹配。

例如，如果起始向量是 *(0, 0, 0)* 而目标向量是 *(10, 0, 10)*:

* 使用 0 的数量会返回 *(0, 0, 0)*
* 使用 0.3 的数量会返回 *(3, 0, 3)*
* 使用 1 的数量会返回 *(10, 0, 10)*

要在你的场景中实现这一点， `lerp()` 我们建议创建一个 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 来存储所需的信息。你还需要定义一个系统，用于在每一帧实现逐步移动。

```ts
// define custom component
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// define system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	}
}

engine.addSystem(LerpMove)

// create entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

![](/docs/_remote/751a298f237c2c997d52fdec.webp)

### 通过系统平滑跟随移动目标

要让实体缓慢朝向持续移动的目标靠近，比如跟随玩家的宠物，可以在每一帧移动剩余距离的一部分。不要使用每一帧固定比例的方式，比如 `Vector3.lerp(current, target, 0.1)`：那样结果会依赖帧率，所以在快的电脑上实体追上目标会比慢的电脑更快。相反，应从 `dt` 中计算出这个比例：

```ts
const follower = engine.addEntity()
Transform.create(follower, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(follower)

const RATE = 5 // Higher values catch up faster

function FollowSystem(dt: number) {
	const transform = Transform.getMutable(follower)
	const target = Transform.get(engine.PlayerEntity).position
	// The same smoothing at any frame rate
	const t = 1 - Math.exp(-RATE * dt)
	transform.position = Vector3.lerp(transform.position, target, t)
}

engine.addSystem(FollowSystem)
```

同样的公式也适用于旋转，使用 `Quaternion.slerp()`.

### 通过系统在两个角度之间旋转

要在两个角度之间平滑旋转，请使用 *slerp* (*球面* 线性插值）算法。这个算法与一个 *lerp*，但它处理的是四元数旋转。

该 `slerp()` 函数接受三个参数：

* 该 [四元数](https://en.wikipedia.org/wiki/Quaternion) 起始旋转的角度
* 该 [四元数](https://en.wikipedia.org/wiki/Quaternion) 目标旋转的角度
* 数量，一个从 0 到 1 的值，表示要执行多少比例的平移。


> [!NOTE]
> **💡 提示**：你可以通过使用 [欧拉](https://en.wikipedia.org/wiki/Euler_angles) 度数（0 到 360）来传入旋转值，使用 `Quaternion.fromEulerDegrees()`.


```ts
const originRotation = Quaternion.fromEulerDegrees(0, 90, 0)
const targetRotation = Quaternion.fromEulerDegrees(0, 0, 0)

let newRotation = Quaternion.slerp(originRotation, targetRotation, 0.6)
```

要在你的场景中实现这一点，我们建议将传入 `Slerp()` 函数的数据存储在一个 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/)中。你还需要定义一个系统，用于在每一帧实现逐步旋转。

```ts
// define custom component
const RotateSlerpData = {
	start: Schemas.Quaternion,
	end: Schemas.Quaternion,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const SlerpData = engine.defineComponent('SlerpData', RotateSlerpData)

// define system
function SlerpRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let slerpData = SlerpData.getMutable(myEntity)
	if (slerpData.fraction < 1) {
		slerpData.fraction += dt * slerpData.speed
		transform.rotation = Quaternion.slerp(
			slerpData.start,
			slerpData.end,
			slerpData.fraction
		)
	}
}

engine.addSystem(SlerpRotate)

// create entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

SlerpData.create(myEntity, {
	start: Quaternion.fromEulerDegrees(0, 0, 0),
	end: Quaternion.fromEulerDegrees(0, 180, 0),
	fraction: 0,
	speed: 0.3,
})
```


> [!WARNING]
> **📔 注**：你也可以改用欧拉角作为 `Vector3` 值，并使用一个 `Lerp()` 函数，但那样就意味着每一帧都要从 `Vector3` 设为 `Quaternion` 转换。旋转值 `Transform` 组件中，因此场景直接使用四元数更高效。


![](/docs/_remote/e4559d5d1a49351c4ded356f.webp)

一种更简单但效率较低的方法是利用 `Quaternion.rotateTowards` 函数，并且不使用任何自定义组件。

```ts
function SimpleRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.rotateTowards(
		transform.rotation,
		Quaternion.fromEulerDegrees(90, 0, 0),
		dt * 10
	)
	如果（
		Quaternion.angle(transform.rotation, Quaternion.fromEulerDegrees(90, 0, 0)) < 0.01
	) {
		console.log('done')
		engine.removeSystem(SimpleRotate)
	}
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
	rotation: Quaternion.fromEulerDegrees(0, 0, 90),
})

MeshRenderer.setBox(myEntity)
```

在上面的示例中 `Quaternion.rotateTowards` 接受三个参数：初始旋转、所需的最终旋转，以及每帧的最大增量。在这种情况下，由于最大增量为 `dt * 10` 度，因此旋转会在大约 9 秒的时间内完成。

注意，该系统还会检查旋转是否完成，如果完成就将该系统从引擎中移除。否则，即使旋转已经完成，系统也会在每一帧继续进行计算。

### 通过系统在两个大小之间改变缩放

如果你希望实体平滑地改变大小且不改变其比例，请使用 *lerp* 的（线性插值）算法 `Scalar` 对象。

否则，如果你想按不同比例改变各轴，请使用 `Vector3` 来表示起始缩放和目标缩放，然后使用 *lerp* 函数的 `Vector3`.

该 `lerp()` 函数的 `Scalar` 对象接受三个参数：

* 起始缩放的数值
* 目标缩放的数值
* 数量，一个从 0 到 1 的值，表示要执行多少比例的缩放。

```ts
const originScale = 1
const targetScale = 10

let newScale = Scalar.lerp(originScale, targetScale, 0.6)
```

要在你的场景中实现这个 lerp，我们建议创建一个自定义组件来存储必要的信息。你还需要定义一个系统，用于在每一帧实现逐步缩放。

```ts
// define custom component
const ScaleTransportData = {
	start: Schemas.Number,
	end: Schemas.Number,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const ScaleTransformComponent = engine.defineComponent(
	'ScaleTransformComponent',
	ScaleTransportData
)

// define system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = ScaleTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const newScale = Scalar.lerp(lerp.start, lerp.end, lerp.fraction)
		transform.scale = Vector3.create(newScale, newScale, newScale)
	}
}

engine.addSystem(LerpMove)

// create entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

ScaleTransformComponent.create(myEntity, {
	start: 1,
	end: 2,
	fraction: 0,
	speed: 1,
})

Vector3.create(1, 1, 1)
```

![](/docs/_remote/3b128506c9be0c97b7264b06.webp)

### 通过系统在两个点之间以不规则速度移动

使用 lerp 方法时，你可以让运动速度变成非线性的。在前面的示例中，我们每一帧按给定数量递增 lerp 值，但我们也可以使用数学函数让数值呈指数增长，或者以其他方式变化，从而得到不同的运动节奏。

你也可以使用会产生周期性结果的函数，比如正弦函数，来描述来回往复的运动。

通常，这些非线性过渡能为场景注入很多活力。沿着曲线加速或逐渐减速的运动，可以很好地表现物体或角色的特性。你甚至可以利用会产生弹跳效果的数学函数。

```ts
// define custom component
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// define system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const interpolatedValue = interpolate(lerp.fraction)
		transform.position = Vector3.lerp(lerp.start, lerp.end, interpolatedValue)
	}
}

// 将 lerp 进度映射到指数曲线
function interpolate(t: number) {
	return t * t
}

engine.addSystem(LerpMove)

// create entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

上面的示例与我们之前展示的线性 lerp 示例非常类似，但 `fraction` 字段在每个 tick 中都映射到一个非线性值。这个非线性值用于计算 `lerp` 函数，从而产生沿指数曲线移动的效果。

你也可以像上面展示的那样，以同样的方式映射旋转或缩放的过渡，只需将线性过渡映射到一条曲线即可。

![](/docs/_remote/95fb7814c57340842f047b62.webp)

### 通过系统沿路径移动

你可以让实体在一组向量数组上循环，逐个执行 lerp 移动，以沿着更复杂的路径前进。

```ts
// define custom component
const PathTransportData = {
	path: Schemas.Array(Schemas.Vector3),
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
	pathTargetIndex: Schemas.Int,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	PathTransportData
)

// define system
function PathMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	} else {
		lerp.pathTargetIndex += 1
		if (lerp.pathTargetIndex >= lerp.path.length) {
			lerp.pathTargetIndex = 0
		}
		lerp.start = lerp.end
		lerp.end = lerp.path[lerp.pathTargetIndex]
		lerp.fraction = 0
	}
}

engine.addSystem(PathMove)

// create entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(1, 1, 1),
})

MeshRenderer.setBox(myEntity)

const point1 = Vector3.create(1, 1, 1)
const point2 = Vector3.create(8, 1, 3)
const point3 = Vector3.create(8, 4, 7)
const point4 = Vector3.create(1, 1, 7)

const myPath = [point1, point2, point3, point4]

LerpTransformComponent.create(myEntity, {
	path: myPath,
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
	pathTargetIndex: 1,
})
```

上面的示例定义了一条由四个 3D 向量组成的 3D 路径。 `PathTransportData` 自定义组件保存了上面示例中的自定义组件所使用的相同数据，但增加了一个 *lerp* 路径 `数组，其中包含路径上的所有点，以及一个` pathTargetIndex `字段，用于跟踪当前正在使用路径的哪个片段。` 系统与

示例中的系统非常相似，但当一个 lerp 动作完成时，它会设置 *lerp* 目标 `起点` 和 `起点` 字段为新值。如果到达路径末尾，就回到路径中的第一个值。

![](/docs/_remote/98be0ad1836095ed41c8b3e5.webp)

## 纹理补间

要让纹理平滑滑动，请使用 `Tween` 组件，并使用 `setTextureMove` 函数。

```ts
Tween.setTextureMove(myEntity, 
	Vector2.create(0, 0), 
	Vector2.create(1, 0), 
	2000
)
```

纹理补间包含以下信息：

* `entity`：要移动其纹理的实体
* `start`：表示起始位置的 Vector2
* `end`：表示结束位置的 Vector2
* `duration`：在两个位置之间移动需要多少毫秒

此外，还有一个可选参数：

* `movementType`：（可选），定义移动是在 offset 字段还是 tiling 字段上进行。默认使用 offset。
* `easingFunction`：要使用哪种缓动函数。请参见 [非线性补间](#non-linear-tweens).

## 恒定纹理移动

要让纹理持续滑动，请使用 `Tween` 组件，并使用 `setTextureMoveContinuous` 函数。

```ts
Tween.setTextureMoveContinuous(myEntity, 
	Vector2.create(0, 1), 
	0.7
)
```

持续纹理补间包含以下信息：

* `entity`：要移动其纹理的实体
* `direction`：表示移动的 Vector2
* `speed`：实体每秒移动多少单位

此外，还有一个可选参数：

* `movementType`：定义移动是在 offset 字段还是 tiling 字段上进行。默认使用 offset。
* `duration`：维持移动多少毫秒。到达此时间后，移动会停止。

要阅读更多关于纹理补间的内容，请参阅 [Texture Tweens](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#texture-tweens) 部分中的三点菜单上移除。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
