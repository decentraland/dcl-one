> 如何设置场景中实体的位置、旋转和缩放

# 实体定位

你可以设置 *位置*, *旋转* 和 *缩放* ，通过使用任何实体的 `Transform` 组件。它可以用于 3D 空间中的任何实体，影响实体的渲染位置。这包括基础图形（立方体、球体、平面等）、3D 文字形状、NFT 形状以及 3D 模型（`GltfContainer`).

## 在 Creator Hub 中使用场景编辑器

当你通过场景编辑器向场景添加项目时，它会隐式包含一个 **Transform** 组件。随后，你通过更改实体的 position、rotation 或 scale 来隐式更改实体的 Transform 组件中的值。你也可以使用场景编辑器的 UI 以数值方式提供这些值，从而获得更高精度。

## 代码要点

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

```ts
// 创建一个新实体
const ball = engine.addEntity()

// 给这个实体一个形状，使其可见
MeshRenderer.setSphere(ball)

// 给这个实体一个 Transform 组件
Transform.create(ball, {
	position: Vector3.create(5, 1, 5),
	scale: Vector3.create(1, 1, 1),
	rotation: Quaternion.Identity(),
})
```

要在一段时间内移动、旋转或调整场景中的实体，请逐帧增量更改该组件上的值。请参见 [移动实体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/) 以了解更多细节和最佳实践。

你省略的任何字段都会采用默认值：位置 `0, 0, 0`，无旋转，以及缩放为 `1, 1, 1`。应省略字段，而不是将其设置为 `undefined`。一个被明确设置为 `undefined`的字段，例如来自一个未设置的变量，会替代默认值，然后场景会在下一次引擎更新时抛出错误，而不是在调用 `Transform.create()`.

```ts
// ❌ 如果 savedRotation 未定义，场景会在之后抛出错误
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation })

// ✅ 回退到一个真实值
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation ?? Quaternion.Identity() })
```


> [!WARNING]
> **📔 注意**: `Vector3` 和 `Quaternion` 必须通过以下方式导入：
> 
> > `import { Vector3, Quaternion } from "@dcl/sdk/math"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


## 位置

`位置` 是一个 *3D 向量*，它设置实体中心在三个轴上的位置， *x*, *y*，以及 *z*。请参见 [几何类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/special-types/) 了解更多详情。

```ts
// 创建一个新实体
const ball = engine.addEntity()

// 使用预定义位置创建 Transform
Transform.create(ball, {
	  position: Vector3.create(5, 1, 5)
})

// 获取 Transform 的可变版本
const mutableTransform = Transform.getMutable(ball)

// 使用对象设置位置
mutableTransform.position = { x: 5, y: 1, z: 5 }

// 使用对象设置位置（另一种语法）
mutableTransform.position = Vector3.create(2, 1, 4)

// 分别设置每个轴
mutableTransform.position.x = 3
mutableTransform.position.y = 1
mutableTransform.position.z = 3
```

设置位置时，请牢记以下注意事项：

* 位置向量中的数字表示 *米* （除非该实体是一个缩放实体的子实体）。
* 由单个地块组成的场景尺寸为 16m x 16m。场景的中心（地面高度）位于 `x:8, y:0, z:8`。如果场景由多个地块组成，则中心会根据它们的排列而变化。
* `x:0, y:0, z:0` 指的是 *西南* 角，即场景基准地块的地面高度位置。

  > 提示：查看场景预览时，场景的 (0,0,0) 点会出现一个指南针，并带有各轴标签作为参考。

  > 注意：你可以通过编辑 `base` 属性来更改场景的基准地块，位于 *scene.json*.
* 为了更好地辨别方向，请使用你的 *左* 手：
  * 你的食指（指向前方）是 *z* 轴
  * 你的中指（指向侧边）是 *x* 轴
  * 你的拇指（指向上方）是 *y* 轴。
* 如果一个实体是另一个实体的子实体，那么 `x:0, y:0, z:0` 指的是其父实体的中心，无论它位于场景中的什么位置。
* 场景中的每个实体都必须始终位于其所占地块的边界内。如果实体超出这些边界，就会抛出错误。

  > 提示：在预览模式下查看场景时，越界的实体会以 *红色*.
* 高亮显示。场景的高度也有限制。无论场景占用多少地块，每个场景的最大高度都是 330 米。放置在 200 米以上的实体可能会出现多人同步问题。请参见 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 了解更多详情。

## 旋转

`旋转` 以 [*四元数*](https://en.wikipedia.org/wiki/Quaternion)存储，即由四个数字组成的系统， *x*, *y*, *z* 和 *w*。这些数字都在 -1 到 1 之间。请参见 [几何类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/special-types/) 了解更多详情。

```ts
// 创建一个新实体
const cube = engine.addEntity()

// 使用预定义的 0 旋转创建 Transform
Transform.create(cube, {
	  rotation: Quaternion.Identity()
})

// 获取 Transform 的可变版本
const mutableTransform = Transform.getMutable(cube)

// 使用欧拉角对象设置旋转
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)

// 使用对象设置旋转
mutableTransform.rotation = { x: 0.1, y: 0.5, z: 0.5, w: 0 }

// 分别设置每个轴
mutableTransform.rotation.x = 0
mutableTransform.rotation.y = 1
mutableTransform.rotation.z = 0.3
mutableTransform.rotation.w = 0
```

你也可以使用以下方式设置 rotation 字段： [*欧拉* 角](https://en.wikipedia.org/wiki/Euler_angles)，这是更常见的 *x*, *y* 和 *z* 表示法，数字范围从 0 到 360，大多数人都更熟悉。要使用欧拉角，请使用以下任一种表示法：

```ts
// 使用欧拉角中的预定义旋转创建 Transform
Transform.create(cube, {
	  rotation: Quaternion.fromEulerDegrees(0, 90, 0)
})

// 获取 Transform 的可变版本
const mutableTransform = Transform.getMutable(cube)

// 使用欧拉角对象设置旋转
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)
```

当使用一个 *3D 向量* 来表示欧拉角时， *x*, *y* 和 *z* 表示该轴上的旋转，以度为单位。完整一圈需要 360 度。

当你读取实体的旋转时，默认返回的是四元数。要获取以欧拉角表示的旋转，请使用 `Quaternion.toEulerAngles()`:

```ts
// 获取 Transform 的只读版本
const transform = Transform.get(cube)

// 获取以欧拉角表示的旋转
const eulerAngle = Quaternion.toEulerAngles(transform.rotation)
```

## 获取实体的全局位置和旋转

该 `getWorldPosition` 和 `getWorldRotation` 函数返回实体的全局位置和旋转。这意味着它返回玩家看到该物品时所处的感知位置或旋转，而忽略任何父级层级。

* `getWorldPosition(engine, entity: Entity): Vector3Type`: 此函数会返回实体的世界位置，会考虑其所有父实体的位置（如果该实体有父实体），并返回 `{x: 0, y: 0, z: 0}` 如果该实体没有 Transform。

```ts
const worldPos = getWorldPosition(engine, childEntity)
console.log(`World position: ${worldPos.x}, ${worldPos.y}, ${worldPos.z}`)
```

* `getWorldRotation(engine, entity: Entity): QuaternionType`: 此函数会返回实体的世界旋转，会考虑其所有父实体的旋转（如果该实体本身有父实体）。它返回一个 `Quaternion` 类型，返回单位四元数 `{x: 0, y: 0, z: 0, w: 1}` 如果该实体没有 Transform。

```ts
const worldRot = getWorldRotation(engine, childEntity)
console.log(`World rotation: ${worldRot.x}, ${worldRot.y}, ${worldRot.z}, ${worldRot.w}`)
```


> [!NOTE]
> **注意：** 全局位置和全局旋转是相对于场景内部坐标的，而不是相对于 Genesis City。


## 面向玩家

添加一个 *Billboard* 组件到实体，使其始终旋转并面向玩家的摄像机。

Billboard 是 90 年代 3D 游戏中的常见技术，其中大多数实体都是始终面向玩家的 2D 平面。同样的思路也可用于旋转 3D 模型。


> [!WARNING]
> **📔 注意**：Billboard 面向 **摄像机**，而不是头像。在第三人称视角中，摄像机位于头像后方并略高，因此两者位置不同，面向摄像机的 Billboard 会明显偏离头像。如果你想让它改为面向头像，请将 `targetEntity` 设置为 `engine.PlayerEntity`.


```ts
// 创建一个新实体
const cube = engine.addEntity()

// 给实体一个可见形状
MeshRenderer.setBox(cube)

// 使用预定义位置创建 Transform
Transform.create(cube, {
	  position: Vector3.create(5, 1, 5)
})

// 给实体一个 Billboard 组件
Billboard.create(cube, {})
```

你可以使用以下参数配置 Billboard 的行为：

* `billboardMode`：使用 `BillboardMode` 的值来设置哪些旋转轴会自动旋转以面向玩家：
  * `BillboardMode.BM_ALL`：实体会在所有旋转轴上转向玩家。如果玩家高于实体，实体会向上看。
  * `BillboardMode.BM_NONE`：实体不会发生任何旋转。
  * `BillboardMode.BM_X`：实体只会在其 *x* 旋转轴上旋转。
  * `BillboardMode.BM_Y`：实体只会在其 *y* 旋转轴上旋转。它只会左右转动，不会上下转动。如果玩家位于实体上方或下方，它会保持与地面垂直。
  * `BillboardMode.BM_Z`：实体只会在其 *z* 旋转轴上旋转。
* `targetEntity`：（可选）一个实体，Billboard 会转向面向它，而不是面向玩家的摄像机。如果所引用的实体不存在（例如尚未创建或已被移除），Billboard 会停止旋转并保持最后的朝向，直到该实体再次存在。将此设置为 `engine.CameraEntity` 等同于不设置它。

```ts
// 平面 Billboard
const perpendicularPlane = engine.addEntity()

Transform.create(perpendicularPlane, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setPlane(perpendicularPlane)

Billboard.create(perpendicularPlane, {
	billboardMode: BillboardMode.BM_Y,
})

// 文本标签
const textLabel = engine.addEntity()

Transform.create(textLabel, {
	position: Vector3.create(6, 1, 6),
})

TextShape.create(textLabel, {
	text: '这段文字始终可读',
})

Billboard.create(textLabel)

// 面向另一个实体而不是玩家的标牌
const sphere = engine.addEntity()

Transform.create(sphere, {
	position: Vector3.create(4, 2, 4),
})

MeshRenderer.setSphere(sphere)

const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: '正在看着球体',
})

Billboard.create(sign, {
	targetEntity: sphere,
})
```


> [!NOTE]
> **💡 提示**：Billboard 非常适合添加到 *文本* 实体上，因为它能让文本始终保持可读。


将 `targetEntity` 设置为 `engine.PlayerEntity` 设置为跟随头像而不是摄像机。这适用于名牌、对话气泡或任何属于头像而非玩家视角的内容：

```ts
// 一个面向头像而不是摄像机的标签
Billboard.create(label, {
	targetEntity: engine.PlayerEntity,
	billboardMode: BillboardMode.BM_Y,
})
```

头像的位置在其脚下，因此头顶上的标签会向下倾斜朝向它。 `BM_Y` 让标签保持直立，并且只会左右转动。

### Billboard 朝向的方式

Billboard 会将其本地 **-Z** 轴指向它所面向的方向。这一点在你将其他实体设为 Billboard 的子实体时很重要：

* 一个位于本地 **+Z** 偏移处的子实体会位于 **后方** ，并被它遮挡。
* 一个位于本地 **-Z** 偏移处的子实体会位于 **前方**，更靠近观察者。

因此，要在 Billboard 标签后面放置边框或背景，请将其在 **+Z**上偏移。要在其前方放置高亮，请将其在 **-Z**.

该 `旋转` 实体的 `Transform` 组件的值不会随着 Billboard 跟随玩家而改变。

如果一个实体同时具有 `Billboard` 组件和 `Transform` 组件，并且带有 `旋转` 值，玩家会看到该实体作为 Billboard 旋转。如果 Billboard 不影响所有轴，其余轴将根据 `Transform` 组件，则抛出异常。


> [!WARNING]
> **📔 注意**进行旋转：如果同一时间有多个玩家在场，每个玩家都会看到朝向自己的 Billboard 实体。Billboard 旋转是针对每个玩家在本地计算的，不会影响其他玩家看到的内容。这不适用于带有 `targetEntity`的 Billboard：由于目标的位置属于场景的一部分，所有玩家看到的这些 Billboard 朝向都相同。


## 面向一组坐标

要让实体 A 看向实体 B：

```
1）从实体 B 中减去实体 A 的位置，得到描述它们之间距离的向量。
2）将该向量归一化，使其长度为 1，并保持其方向不变。
3）使用 `Quaternion.lookRotation` 获取一个描述朝该方向旋转的四元数。
4）将该四元数设置为实体 A 的旋转
```

```ts
export function turn(entity: Entity, target: Vector3.ReadonlyVector3) {
	const transform = Transform.getMutable(entity)
	const difference = Vector3.subtract(target, transform.position)
	const normalizedDifference = Vector3.normalize(difference)
	transform.rotation = Quaternion.lookRotation(normalizedDifference)
}
```

## 缩放

`缩放` 也是一个 *3D 向量*，存储为一个 `Vector3` 对象，包括 *x*, *y* 和 *z* 轴上的缩放因子。实体的形状会相应缩放，无论它是基础图形还是 3D 模型。

默认缩放为 1，因此将值设为大于 1 可拉伸实体，设为小于 1 可缩小实体。

```ts
// 创建一个新实体
const ball = engine.addEntity()

// 使用预定义位置创建 Transform
Transform.create(ball, {
	  scale: Vector3.create(5, 5, 5)
})

// 获取 Transform 的可变版本
const mutableTransform = Transform.getMutable(ball)

// 使用 Vector3 设置缩放

mutableTransform.scale = Vector3.create(2, 2, 2)

// 使用对象设置位置
mutableTransform.scale = { x: 5, y: 1, z: 5 }

// 分别设置每个轴
mutableTransform.scale.x = 3
mutableTransform.scale.y = 3
mutableTransform.scale.z = 2
```

## 继承父级的变换

当一个实体嵌套在另一个实体中时，子实体会继承父实体的组件。这意味着如果父实体被定位、缩放或旋转，其子实体也会受到影响。子实体的 position、rotation 和 scale 值不会覆盖父实体的值，而是会叠加。

你可以通过设置子实体上的 `parent` 字段来将一个实体设为另一个实体的父实体。 `Transform` 组件，则抛出异常。

如果父实体被缩放，其所有子实体的位置值也会一起缩放。

```ts
// 创建实体
const parentEntity = engine.addEntity()
const childEntity = engine.addEntity()

// 为父实体创建 Transform
Transform.create(parentEntity, {
	position: Vector3.create(3, 1, 1),
	scale: Vector3.create(0.5, 0.5, 0.5),
})

// 为子实体创建 Transform，并将其设为子实体
Transform.create(childEntity, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity,
})
```

在这个示例中，由于父实体具有该缩放，子实体会被缩小到 0.5。子实体的位置也会相对于其父实体。我们必须将父实体的位置与子实体的位置相加。在这种情况下，由于父实体缩小为原来的一半，子实体的变换也会按比例缩小。按绝对坐标来说，子实体位于 `{ x: 3, y: 1.5, z: 1 }`。如果父实体有一个 `旋转`，这也会影响子实体的最终位置，因为它会改变子实体偏移的轴。

如果子实体的 Transform 上没有 `位置` ，默认值是 `0,0,0`，这会使其位置保持与父实体相同。

你可以使用一个没有形状组件的不可见实体作为父实体，用来包裹一组其他实体。该实体不会在渲染场景中可见，但可用于将其子实体分组，并对它们全部应用变换。


> [!WARNING]
> **📔 注意**：避免在层级结构中创建循环，也就是某个实体最终成为自己的祖先。SDK 会检测这些循环，并记录一条警告，指出涉及的实体。在你修复父子关系之前，受影响的实体不会被正确定位。


## 将实体附加到头像

有三种方法可以将实体附加到玩家：

* 将其设为的子实体 **头像实体**
* 将其设为的子实体 **摄像机实体**
* 使用 **AvatarAttach 组件**

将实体附加到头像的最简单方法，是将父实体设置为 [保留实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities) `engine.PlayerEntity`。这样实体就会随玩家的位置一起移动。

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.PlayerEntity,
})
```

你也可以将实体设置为 [保留实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities) `engine.CameraEntity`。在第一人称中使用相机实体时，附加实体会跟随相机的移动。这非常适合让某个物体始终保持在视野中，例如让枪的 3D 模型始终可见，即使相机朝上也一样。

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.CameraEntity,
})
```

要将对象附加到 avatar 的某根骨骼上，并让它随 avatar 的动画一起移动，请添加一个 `AvatarAttach` 组件到实体。

你可以在 avatar 上选择不同的锚点，其中大多数锚点都与玩家的骨架相连，并会跟随玩家的动画。例如，使用右手锚点时，附加实体会在 avatar 挥手或奔跑时摆动手臂而移动，就像玩家正把该实体拿在手中一样。

```ts
// 附加到主玩家，如果未设置 avatarId，则默认使用 engine.PlayerEntity
AvatarAttach.create(myEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// 通过 ID 附加到玩家
AvatarAttach.create(myEntity, {
	avatarId: '0xAAAAAAAAAAAAAAAAA',
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})
```

在创建一个 `AvatarAttach` 组件时，请传入一个包含以下数据的对象：

* `avatarId`: *可选* 要附加的玩家 ID。对于通过 Ethereum 钱包连接的玩家，这与玩家的 Ethereum 地址相同。如果未指定，则会将实体附加到本地玩家的 avatar 上。
* `anchorPointId`：要将实体附加到 avatar 骨骼上的哪个锚点，请使用枚举中的值 `AvatarAnchorPointType`.


> [!WARNING]
> **📔 注意**：如果你希望场景中的所有玩家都看到附着在同一个玩家身上的物体，例如让所有人都看到玩家 A 捡起了一个物体并用左手拿着它，那么你必须为 `avatarId`提供一个值。如果未指定，则所有玩家都会看到该物体附着在自己的 avatar 上。


下面的示例会放置一个附加到特定 avatar 上的实体，让所有其他玩家都看到它附着在同一个 avatar 上。

```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { AvatarAnchorPointType, AvatarAttach, engine, Entity } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'

async function attachToPlayer(){

 let userData = await getPlayer()
 console.log(userData)

 if (!userData || !userData.wearables) return

  let entity = engine.addEntity()

  AvatarAttach.create(entity, {
    avatarId: userData.userId,
    anchorPointId: AvatarAnchorPointType.AAPT_RIGHT_HAND,
  })

  // 其他组件

  syncEntity(entity, [AvatarAttach.componentId])

}
```

以下锚点可用于 `AvatarAnchorPointType` 枚举：

* `AAPT_RIGHT_HAND`：固定在玩家的右手上
* `AAPT_LEFT_HAND`：固定在玩家的左手上
* `AAPT_HEAD`：固定在玩家头部中心。
* `AAPT_NECK`：固定在玩家颈部底部。
* `AAPT_SPINE`：固定在脊柱上部。
* `AAPT_SPINE1`：固定在脊柱中部。
* `AAPT_SPINE2`：固定在脊柱下部。
* `AAPT_HIP`：固定在髋骨上。
* `AAPT_LEFT_SHOULDER`：固定在左肩上。
* `AAPT_LEFT_ARM`：固定在左上臂骨上，位于肩部高度。
* `AAPT_LEFT_FOREARM`：固定在左前臂骨上。
* `AAPT_LEFT_HAND_INDEX`：固定在左手食指指尖。
* `AAPT_RIGHT_SHOULDER`：固定在右肩上。
* `AAPT_RIGHT_ARM`：固定在右上臂骨上，位于肩部高度。
* `AAPT_RIGHT_FOREARM`：固定在右前臂骨上。
* `AAPT_RIGHT_HAND_INDEX`：固定在右手食指指尖。
* `AAPT_LEFT_UP_LEG`：固定在左腿上部的腿骨上。
* `AAPT_LEFT_LEG`：固定在左腿下部的腿骨上。
* `AAPT_LEFT_FOOT`：固定在左腿的脚踝上。
* `AAPT_LEFT_TOE_BASE`：固定在左腿脚趾尖。
* `AAPT_RIGHT_UP_LEG`：固定在右腿上部的腿骨上。
* `AAPT_RIGHT_LEG`：固定在右腿下部的腿骨上。
* `AAPT_RIGHT_FOOT`：固定在右腿的脚踝上。
* `AAPT_RIGHT_TOE_BASE`：固定在右腿脚趾尖。
* `.AAPT_NAME_TAG`：浮动在玩家名牌正上方，不受玩家动画影响。

  > 注意：名牌高度会根据玩家所穿戴物品的高度动态调整。因此，佩戴高帽子的玩家，其名牌会比其他人稍微高一些。
* `AAPT_POSITION` *已弃用*：玩家的整体位置。它会显示在玩家脚下方 0.8 的高度处。

  >


> [!WARNING]
> \> \*\*📔 注\*\*：\`AAPT\_POSITION\` 已弃用。要跟随玩家的整体位置，最好让实体成为 Avatar Entity 的子实体。示例请参见本节开头。 >



> [!NOTE]
> **💡 提示**：要使用这些值，请输入 `AvatarAnchorPointType。` ，VS Code 会在下拉菜单中显示完整的可选列表。


![](/docs/_remote/a3894bbddd0ce5df1254538a.webp)

实体渲染由场景中每个实例本地决定。将实体附加到某个玩家身上，并不会让其他正在看到该玩家的玩家也看到它。如果实体附加到默认的本地玩家上，那么每个玩家都会将该实体视为附加在自己的 avatar 上。


> [!WARNING]
> **📔 注意**：附加到 avatar 上的实体必须保持在场景边界内才能被渲染。如果玩家走出你的场景，任何附加的实体都会停止渲染，直到玩家重新走回场景中。智能穿戴物没有这个限制。


该 `AvatarAttach` 组件会覆盖其中的 `Transform` 组件。你放入的任何值都会被 `Transform` 组件中实体相对于玩家 Transform 的相对位置所替换，这些值会随着玩家移动和做动画而逐帧更新。

如果你需要让实体相对于 avatar 上的锚点有偏移，或需要不同的旋转或缩放，请通过父实体来实现。

1. 创建一个不可见实体，只包含一个 `Transform` 和一个 `AvatarAttach` 组件。其 `Transform` 值会随着玩家移动而被覆盖
2. 将你想要附加的实体设为该父实体的子实体。它的 `Transform` 值可以描述相对于锚点的偏移。

```ts
// 创建父实体
const parentEntity = engine.addEntity()

// 将父实体附加到玩家
AvatarAttach.create(parentEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// 创建子实体
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: parentEntity,
})
```


> [!WARNING]
> **📔 注意**：如果附加的实体带有碰撞体，这些碰撞体可能会阻挡玩家移动或造成抖动效果。你可能需要禁用附加实体碰撞体的物理层。请参见 [碰撞层](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#collision-layers)



> [!WARNING]
> **📔 注意**：如果你有一个子实体，并且想知道它的全局位置和/或旋转，可以使用 `getWorldPosition` 和 `getWorldRotation` 函数。你可以在 [获取全局位置和旋转](#getting-global-position-and-rotation-of-an-entity) 部分中查看。


### 附加到其他玩家

你可以使用 `AvatarAttach` 组件将实体附加到其他玩家。要做到这一点，你必须知道该玩家的 ID。

要将实体附加到另一位玩家的 avatar 上，你必须在字段中提供用户 ID `avatarId`。有 [多种方式](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data) 来获取这些数据。


> [!WARNING]
> **📔 注意**：对于通过 Ethereum 钱包连接的玩家，他们的 `userId` 与他们的 Ethereum 地址相同。


获取 `userId` 以查看所有其他附近玩家，可通过 `getPlayer()`

```ts
executeTask(async () => {
	for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		console.log('玩家 ID：', data.address)
	}
})
```

与 `AvatarAttach`，你可以使用以下代码为场景中每个其他玩家的头顶添加一个漂浮的立方体：

```ts
executeTask(async () => {
        for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
            const myEntity = engine.addEntity()
            MeshRenderer.setBox(myEntity)
            AvatarAttach.create(myEntity, {
                anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND,
                avatarId: data.address,
            })
        }
    })
```

请参阅以下内容了解获取其他用户 ID 的其他方法： [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data).


> [!WARNING]
> **📔 注意**：不要将另一位玩家的实体设置为 `parent` 你实体的。引擎会写入该实体的 `Transform` 供你的场景读取，但它并不是你的实体可以挂接的父级，因此子实体会退回到场景根节点并停留在场景原点。请使用 `AvatarAttach` 上面所示的玩家地址来代替。


随着玩家来来去去，玩家实体 ID 会被重复使用，因此同一个实体以后可能属于不同的人。如果你为每个玩家保留一个标记，请存储它所对应的地址并在每一帧进行比较。当实体上的地址发生变化时，删除旧标记并创建新标记：

```ts
import { AvatarAttach, AvatarAnchorPointType, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

// 标记所对应的地址，这样我们就能识别被重复使用的实体 ID
const markers = new Map<Entity, { address: string; marker: Entity }>()

function markerSystem() {
	// 移除玩家已离开，或其实体 ID 现在属于别人的标记
	for (const [player, entry] of markers) {
		if (PlayerIdentityData.getOrNull(player)?.address === entry.address) continue
		engine.removeEntity(entry.marker)
		markers.delete(player)
	}

	for (const [player, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		if (markers.has(player)) continue
		const marker = engine.addEntity()
		AvatarAttach.create(marker, {
			avatarId: data.address,
			anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
		})
		markers.set(player, { address: data.address, marker })
	}
}

engine.addSystem(markerSystem)
```

## 场景边界

场景中的所有实体都必须位于场景边界内，因为边界之外的区域属于其他玩家拥有的地块。

如果在预览运行时你的模型有任何部分超出这些限制，超出的部分将会被裁掉并且不会渲染，无论是在预览中还是在已发布的场景中。

场景中实体的位置在移动时会持续检查，如果实体离开场景后又返回，它会先被移除，然后再恢复正常渲染。

场景地面上的网格显示了场景的边界，默认情况下其范围从 0 到 16 的 *x* 和 *z* 轴上，并且最多到 20 的 *y* 轴。你可以自由地将实体放置在地下，在 0 以下的 *y* 轴。


> [!NOTE]
> **💡 提示**：如果你的场景需要更多地块，你可以在项目的 `scene.json` 文件中添加。请参见 [场景元数据](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/) 获取说明。添加后，你应该会看到网格延伸以覆盖额外的地块。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
