# 碰撞体

了解为实体提供 3D 形状和碰撞的不同组件。

具有碰撞器的实体会占据空间并阻挡玩家路径，没有碰撞器的实体则可以被玩家的角色穿过。

要让实体可点击，也需要碰撞器。按钮事件基于实体的碰撞器形状，而不是其可见形状。

有独立的碰撞层，可分别用于与玩家物理交互或与指针事件交互；可以将碰撞器配置为只与其中一种交互。它们也可以配置为与自定义层交互，这些层可与 [射线投射](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/) 来处理最适合场景的内容。


> [!WARNING]
> **📔 注意**：碰撞器不会影响其他实体之间的交互，实体始终可以重叠。碰撞设置只会影响实体与玩家角色以及按钮事件的交互。Decentraland 没有原生物理引擎，所以如果你想让实体下落、碰撞或弹跳，必须把这种行为编写到场景中，或者导入一个库来处理。


## 使用场景编辑器

管理实体碰撞器最简单的方法是使用 [场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/).

你可以添加一个 **网格碰撞器** 组件到你的实体，为其指定一个基础形状（立方体、平面、球体、圆柱体或圆锥体）。然后你可以选择 [碰撞层](#collision-layers) ，从下拉菜单中选择。

你还可以在一个 **GLTF** 组件上配置碰撞层，以更改默认的 [碰撞层](#collision-layers) ，该默认值用于模型的碰撞几何体或可见几何体。参见 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components).

![](/docs/_remote/d21ae0005c271ae9c97a8144.webp)

## 基础形状上的碰撞器

该 `MeshCollider` 组件会根据基础形状（盒子、球体、平面、圆柱体或圆锥体）为实体添加一个简单碰撞器。

具有一个 `MeshRenderer` 组件来赋予它们一个 [基础形状](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/#primitive-shapes) 默认没有碰撞器。你还必须给实体添加一个 `MeshCollider` 组件。

以下碰撞器形状可在 `MeshCollider`中使用。其中一些形状包含可选的附加字段，且这些字段特定于该形状。

* **盒子**:

  使用 `MeshCollider.setBox()`，并传入实体。
* **平面**:

  使用 `MeshCollider.setPlane()`，并传入实体。
* **球体**:

  使用 `MeshCollider.setSphere()`，并传入实体。
* **圆柱体**:

  使用 `MeshCollider.setCylinder()`，并传入实体。传入 `radiusTop` 和 `radiusBottom` 作为额外的可选字段，以修改圆柱体。


> [!NOTE]
> **💡 提示**：将其中任意一个 `radiusTop` 或 `radiusBottom` 设为 0 即可变成圆锥体。


这个示例定义了一个不能被穿过的盒子实体。

```ts
// 创建实体
const myCollider = engine.addEntity()

// 可见形状
MeshRenderer.setBox(myCollider)

// 碰撞器
MeshCollider.setBox(myCollider)
```

所使用的形状 `MeshCollider` 不一定需要与所使用的形状相匹配 `MeshRenderer`。你还可以向一个 `MeshCollider` 添加到一个具有来自一个 `GltfContainer` 组件的 3D 模型的实体上，或者添加到完全没有可见形状的实体上。


> [!WARNING]
> **📔 注意**： `MeshCollider` 组件和 `ColliderLayer` 必须通过
> 
> `import { MeshCollider, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


## 3D 模型上的碰撞器

可以在两个不同的几何层级上为 3D 模型分配碰撞器：

* `visibleMeshesCollisionMask`：指模型的可见几何体。默认情况下，该几何体没有碰撞器。
* `invisibleMeshesCollisionMask`：指碰撞网格，其名称以 `_collider`结尾。默认情况下，该几何体会被视为用于物理和指针事件的碰撞器。

嵌入为 3D 模型一部分的任何网格，只要其名称以 `_collider` 结尾，都会被视为 `invisibleMeshesCollisionMask` 层的一部分，并默认被解释为碰撞器。

将碰撞几何体定义为单独的不可见层，可以实现更精细的控制，而且比使用可见几何体对系统的要求低得多，因为碰撞对象通常比原始模型简单得多（顶点更少）。

如果模型没有任何碰撞几何体，而你想让它影响物理或指针事件系统，你可以：

* 通过 `visibleMeshesCollisionMask`.


> [!WARNING]
> **📔 注意**：如果对象的可见几何体有很多顶点，请注意这可能会带来更高的性能开销。


* 给实体添加一个 `MeshCollider` 组件，以赋予它一个基础形状碰撞器。
* 叠加一个具有 `MeshCollider` 组件。
* 在 Blender 等外部工具中编辑模型，以包含一个 *碰撞网格*。碰撞器必须命名为 *x\_collider*，其中 *x* 是模型的名称。因此，对于名为 *house*，碰撞器必须命名为 *house\_collider*.

你可能还想将指针事件碰撞层分配给 `visibleMeshesCollisionMask` ，如果你希望悬停提示和指针事件更准确地响应实体轮廓。请注意，这会带来更高的性能开销。


> [!WARNING]
> **📔 注意**：确保不要将同一层（物理、指针事件或自定义层）同时分配给 `visibleMeshesCollisionMask` 和 `invisibleMeshesCollisionMask`，因为这会非常低效地使用资源。你可以让两者使用不同的层，例如让物理使用不可见层，而指针事件使用可见层。


```ts
// 创建实体
const myEntity = engine.addEntity()

// 分配 GLTF 形状
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})
```

参见 [3D 模型](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) ，了解如何向 3D 模型添加碰撞用的不可见几何体的更多细节。


> [!WARNING]
> **📔 注意**： `GltfContainer` 组件和 `ColliderLayer` 必须通过
> 
> > `import { GltfContainer, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


### 动画模型

当为包含 [基于骨骼的动画](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/animations/)的模型设置碰撞器以使用可见几何体时，碰撞器不会跟随动画。碰撞网格会保持其原始形状。如果动画涉及变形网格的几何体，那么在动画播放时，碰撞网格会保留未动画化的形状。

当播放涉及整体移动网格但不改变其形状的动画时，这些变化会被碰撞器准确反映。例如，如果平台作为动画的一部分移动，那么平台的碰撞器也会随动画移动。

## 碰撞层

场景可以处理具有不同行为的独立碰撞层。

你可以配置一个 `MeshCollider` 组件或 `GltfContainer` 组件，使其只响应一种交互、几种交互或不响应任何交互。为此，在 `MeshCollider` 设置 `collisionMask` 属性，并在 `GltfContainer` 设置 `visibleMeshesCollisionMask` 或 `invisibleMeshesCollisionMask` 属性中设置为以下一个或多个值：

* `ColliderLayer.CL_PHYSICS`：阻挡玩家移动（场景中的墙壁、地板、平台）。不影响指针事件。
* `ColliderLayer.CL_POINTER`：仅响应指针事件。不阻挡玩家移动。
* `ColliderLayer.CL_PLAYER`：将该碰撞器标记为头像标记。以 `CL_PLAYER` 为目标的射线投射和触发区域会检测到它，但玩家胶囊会直接穿过它（不会产生物理阻挡）。在场景网格上，这适合将某个网格标记为“仅用于检测的类似头像目标”。
* `ColliderLayer.CL_MAIN_PLAYER`：类似于 `CL_PLAYER`，但仅针对本地玩家。带有 `CL_MAIN_PLAYER` 的射线投射和触发区域会检测到它；远程头像的射线投射/触发区域则不会。
* `ColliderLayer.CL_CUSTOM1` 一直到 `CL_CUSTOM8`：可与射线投射和触发区域一起使用，仅检测特定的自定义层碰撞。
* `ColliderLayer.CL_NONE`：不响应任何类型的碰撞。


> [!NOTE]
> **💡 提示**: `CL_PLAYER` 和 `CL_MAIN_PLAYER` 在一个 `MeshCollider` / `GltfContainer` 是 **仅检测** 层——玩家胶囊会穿过它们。如果你希望网格既能作为头像被检测到，又能在物理上阻挡玩家，请将头像层与 `CL_PHYSICS` （例如 `CL_PHYSICS | CL_MAIN_PLAYER`).



> [!WARNING]
> **📔 注意**：要禁用来自 `MeshCollider` 组件的碰撞，请删除该组件。不要将碰撞层设置为 `ColliderLayer.CL_NONE`。 `MeshCollider` 组件。它存在一个已知问题。它不会禁用所有碰撞，而是将该值视为默认值（`ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`).


```ts
// 创建实体
const myEntity = engine.addEntity()
// 可见形状
MeshRenderer.setBox(myEntity)

// 创建一个仅响应玩家物理的 MeshCollider 组件
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)
```

单个碰撞掩码可以响应多个碰撞层。使用 `|` 字符作为一个 *或*，即可包含所需数量的层。MeshCollider 的默认值为 `ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`.

```ts
MeshCollider.setBox(
	myEntity,
	ColliderLayer.CL_CUSTOM1 |
		ColliderLayer.CL_CUSTOM3 |
		ColliderLayer.CL_PHYSICS |
		ColliderLayer.CL_POINTER
)
```

你可以使用这 8 个不同的自定义层来满足场景最适合的需求，例如一个可用于 NPC 的视线计算，而另一个可用于估算坠落物体的轨迹。为不同系统使用不同的层可以减少资源消耗，因为在每种情况下，你只需检查与相关实体的碰撞。

参见 [射线投射](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting/) ，了解如何使用自定义碰撞层的更多信息。

### 摄像机与碰撞器

当玩家摄像机以第三人称模式移动时，摄像机可能会被碰撞器阻挡，也可能不会，这取决于分配给实体的碰撞层。在设计场景时要注意这一点，你可能希望阻止摄像机穿过墙壁或其他实体。

要避免摄像机穿过墙壁，你必须同时将 `ColliderLayer.CL_PHYSICS` 和 `ColliderLayer.CL_POINTER` 层分配给你想要阻挡摄像机的实体。重要的是，这两个层都要分配给实体上的同一几何体。因此，如果你将 `ColliderLayer.CL_PHYSICS` 层分配给实体的可见层，你还必须将 `ColliderLayer.CL_POINTER` 层分配给同一几何体。

例如，在 Creator Hub 中，以下设置组合将阻止摄像机穿墙：

![](/docs/_remote/1b4a254a3e56b4b32b988fb4.webp)

这两个 `ColliderLayer.CL_PHYSICS` 和 `ColliderLayer.CL_POINTER` 层都分配给实体几何体的同一个不可见层。如果它们都分配给可见层，结果也会相同。这是默认行为，无论是通过 Creator Hub 还是通过代码添加实体时都是如此。

![](/docs/_remote/6d4572b10a1972901a7ea232.webp)

在第二个示例中，摄像机可以穿过墙壁，因为 `ColliderLayer.CL_PHYSICS` 层分配给了实体的不可见层，而 `ColliderLayer.CL_POINTER` 层分配给了实体的可见层，即使两个几何体具有相同的整体形状。

```ts
// 不允许摄像机穿墙
// 默认（指针和物理都使用不可见几何体）
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// 不允许摄像机穿墙
// 两者都使用相同的不可见几何体
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// 不允许摄像机穿墙
// 两者都使用相同的可见几何体
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	visibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// 允许摄像机穿墙
// 物理和指针位于不同层
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// 允许摄像机穿墙
// 物理和指针位于不同层
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
	visibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
})
```

### 指针阻挡

只有具有碰撞器的形状才能通过 [指针事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)。实体还需要有碰撞器，以阻止指针事件穿过它并避免点击到其后面的实体。例如，如果箱子周围有碰撞器，玩家就无法拾取锁在箱子里的东西。玩家的指针事件只会受到在 `ColliderLayer.CL_POINTER` 层中处于激活状态的网格影响。

默认情况下，MeshCollider 会同时影响物理层和指针层，但你可以将其改为只影响其中一个，或者两个都不影响，也可以改为影响自定义层。


> [!WARNING]
> **📔 注意**：除了碰撞器之外，实体还需要有一个 `PointerEvents` 组件才能响应指针事件。 `pointerEventsSystem` 辅助函数也会处理这一需求。


```ts
// 仅响应玩家物理
// 例如一个你不能穿过但可以点击穿过的隐形墙
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)

// 仅响应玩家的指针
// 例如一个你可以点击拾取但可以直接穿过去的物品
MeshCollider.setBox(myEntity2, ColliderLayer.CL_POINTER)
```

默认情况下，一个 `GltfContainer` 的可见几何体不会映射到任何碰撞层，但不可见几何体会同时影响物理层和指针层。你可以将其改为只影响其中一个，或者两个都不影响，也可以改为影响自定义层。你也可以用同样的方式配置可见几何体层。

```ts
// 默认（指针和物理都使用不可见几何体）
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// 玩家物理使用更简单的不可见几何体
// 指针事件使用可见几何体完整而细致的轮廓
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// 玩家物理和指针事件都使用可见几何体完整而细致的轮廓
// 较简单的不可见几何体映射为 ColliderLayer.CL_NONE，以避免同时计算两者
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
	visibleMeshesCollisionMask:
		ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS,
})

// 无论是可见几何体还是不可见几何体，都不响应任何类型的碰撞：
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
})
```

## 高级 MeshCollider 语法

创建一个 `MeshCollider` 组件、且不使用任何辅助函数简化时，完整语法如下：

```ts
MeshCollider.create(myBox, {
	mesh: {
		$case: 'box',
		box: {},
	},
})

MeshCollider.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: {},
	},
})

MeshCollider.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshCollider.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

基础协议就是这样解释 MeshCollider 组件的。辅助函数将其抽象出来并提供更友好的语法，但在幕后它们输出的仍是这种语法。

该 `$case` 字段允许你指定一种允许的类型。每种类型支持不同的一组参数。

支持的 `$case` 如下：

* `盒子`
* `平面`
* `球体`
* `圆柱体`

根据 `$case`的值，可以为相应的形状定义对象，并传入任何相关属性。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
