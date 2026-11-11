> 了解为实体提供 3D 形状和碰撞的不同组件。

# 形状组件

Decentraland 中的三维场景基于 [实体-组件](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) 模型，其中场景中的一切都是一个 *实体*，且每个实体都可以包含 *组件* ，这些组件决定其特性和功能。

实体渲染出的形状由其使用的组件决定。

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

## 在 Creator Hub 中使用场景编辑器

为实体添加形状的最简单方法是使用场景编辑器。你可以添加一个 **网格渲染器** 组件来提供一个基本形状，或者添加一个 **GLTF** 组件来引用文件中的 3D 模型。请参阅 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components).

## 基本形状

几种基本形状，通常称为 *原语*，可以通过为实体添加一个 `MeshRenderer` 组件的实体时，组件不会被激活。

可用的形状如下。部分形状包含仅适用于该形状的可选附加字段。

* **立方体**:

  使用 `MeshRenderer.setBox()`，并传入该实体。传入 `uvs` 作为额外可选字段，以映射纹理对齐方式。请参阅 [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/) 以了解更多详情。
* **平面**:

  使用 `MeshRenderer.setPlane()`，并传入该实体。传入 `uvs` 作为额外可选字段，以映射纹理对齐方式。请参阅 [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/) 以了解更多详情。
* **球体**:

  使用 `MeshRenderer.setSphere()`，并传入该实体。
* **圆柱体**:

  使用 `MeshRenderer.setCylinder()`，并传入该实体。传入 `顶部半径` 和 `底部半径` 作为额外可选字段，用于修改圆柱体。

  提示：将 `顶部半径` 或 `底部半径` 中的任意一个设为 0，即可变成圆锥体。

以下示例创建一个立方体：

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)
```

以下示例创建一个带有 `顶部半径` 为 0 的圆柱体，这会生成一个圆锥体：

```ts
const myCone = engine.addEntity()

Transform.create(myCone, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setCylinder(myCone, 1, 0)
```

基本形状不包含材质。若要为其添加颜色或纹理，你必须分配一个 [材质组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/) 给同一个实体。

若要让基本形状可点击，或防止玩家穿过它，你必须给该实体添加一个 *碰撞体* 通过一个 [MeshCollider](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/) 组件的实体时，组件不会被激活。

若要更改一个已经具有 `MeshRenderer` 组件的实体的形状，请运行 `MeshRenderer.setBox()` 或其他任意辅助函数，它会覆盖原来的形状。无需移除原来的 `MeshRenderer` ，也无需使用高级语法。

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)

// 覆盖形状
MeshRenderer.setSphere(myCube)
```


> [!WARNING]
> **📔 注意**： `MeshRenderer` 组件必须通过以下方式导入：
> 
> > `import { MeshRenderer } from "@dcl/sdk/ecs"`
> 
> 请参阅 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些情况。


## 3D 模型

对于更复杂的形状，你可以使用 Blender 等外部工具创建 3D 模型，然后将其导入到 *.glTF* 或 *.glb* （二进制 *.glTF*). [glTF](https://www.khronos.org/gltf) （GL Transmission Format）是 Khronos 发起的一个开放项目，为 3D 资源提供一种通用、可扩展的格式，既高效又能与现代 Web 技术高度互操作。

要将外部模型添加到场景中，请添加一个 `GltfContainer` 组件到实体，并将其 `src` 设置为包含该模型的 glTF 文件路径。

```ts
const houseEntity = engine.addEntity()

GltfContainer.create(houseEntity, {
	src: 'models/House.gltf',
})
```

该 `src` 字段是必需的，因此在构造该组件时必须为其赋值。在上面的示例中，该模型位于场景项目文件 `models` 夹中。


> [!NOTE]
> **💡 提示**：我们建议将模型单独放在场景中的 `assets/scene/models` 文件夹内。


glTF 模型可以包含其自身嵌入的纹理、材质、碰撞体和动画。请参阅 [3D 模型](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) 以了解更多信息。若要覆盖模型的材质，请使用 [GltfNodeModifiers](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 组件遍历所有玩家。参见 [修改 glTF 材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 以了解更多详情。

若要防止玩家穿过 3D 模型，或让模型可点击，你必须拥有一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)，它可以嵌入在模型中，或通过一个 `MeshCollider` 组件的实体时，组件不会被激活。

请记住，所有模型、其着色器和纹理都必须符合 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/).


> [!WARNING]
> **📔 注意**： `GltfContainer` 组件必须通过以下方式导入：
> 
> `import { GltfContainer } from "@dcl/sdk/ecs"`
> 
> 请参阅 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些情况。


### 预加载 3D 模型

在某些情况下，自定义 3D 模型会被添加到场景中，但不会立即使用。例如，自定义椅子模型可能只会在玩家与另一个对象交互时才加载。在这种情况下，第一次交互可能需要一些时间，直到模型完成下载和加载。

为避免这种情况，请使用 `AssetLoad.create` 方法，以确保资源在需要之前已下载完成。

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

更多信息请查看 [预加载资源](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/pre-load-resources/) 文档。

### 免费的 3D 模型库

你也可以从一些免费或付费资源库下载 3D 模型，而不必自己制作。

为了帮助你入门，下面列出了一些提供免费或价格相对低廉内容的资源库：

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [来自 Builder 的资源](https://github.com/decentraland/builder-assets/tree/master/assets)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/) （主要用于 3D 打印的 3D 模型，但也可适用于虚拟世界）
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://www.cgtrader.com/)


> [!WARNING]
> **📔 注意**：请注意你下载内容所附带的许可限制。


请注意，在其中一些网站上，你可以选择模型的下载格式。始终选择 *.glTF* 格式，如果可用的话。若不可用，则你必须将其转换为 *glTF* 之后才能在场景中使用它们。为此，我们建议将它们导入 Blender，并从中导出为 *.glTF* 。

### 优化 3D 模型

为了确保场景中的 3D 模型加载更快并占用更少内存，请遵循以下最佳实践：

* 将模型保存为 *.glb* 格式，这是 *.gltf*.
* 如果有多个模型共享相同的纹理，请将带纹理的模型导出到单独文件中。这样多个模型就可以引用同一个纹理文件，而该文件只需加载一次。
* 如果你的场景中有会出现和消失的实体，最好将这些实体进行池化，并把它们放在地下，或将缩放设为 0。这样可以让它们更快出现，代价是它们在未使用时仍会占用内存。

## 拉伸形状

基本形状和 3D 模型都有默认尺寸，你可以通过修改实体的 `变换` 组件的实体时，组件不会被激活。

```ts
const primitiveEntity = engine.addEntity()

MeshRenderer.setBox(primitiveEntity)

Transform.create(primitiveEntity, {
	position: { x: 8, y: 1, z: 8 },
	scale: { x: 4, y: 0.5, z: 4 },
})
```

## 设为不可见

你可以通过给实体添加一个 `VisibilityComponent`，并将其 `visible` 属性设为 *false*.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(myEntity)

VisibilityComponent.create(myEntity, { visible: false })
```

该 `VisibilityComponent` 对具有基本形状的实体以及具有 `GLTFContainer` 组件的实体同样适用。

如果实体是不可见的，其碰撞体仍可能阻挡玩家路径，和/或阻止点击位于其后方的实体，这取决于分配给该碰撞体的碰撞层。

### 传播可见性

你可以使用 `propagateToChildren` 字段在 `VisibilityComponent` 上，用于将配置应用到实体子节点树中的每个子节点。若 `propagateToChildren` 被设置为 *true*，这些设置会影响向下所有层级的子节点。这可以为你省去大量逐个标记子实体为不可见或可见的繁琐工作。

```ts
// 父实体（显式设为不可见）
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(parentEntity)
VisibilityComponent.create(parentEntity, { visible: false, propagateToChildren: true })

// 子实体（由于父实体而隐式不可见）
const child = engine.addEntity()
Transform.create(child, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity
})
MeshRenderer.setBox(child)
```


> [!WARNING]
> **📔 注意**：如果实体有自己的 `VisibilityComponent`，则会覆盖来自父级的任何配置。
> 
> 如果实体没有自己的 `VisibilityComponent`，那么其可见性将由其最近的、带有 `VisibilityComponent` 和 `propagateToChildren` 设置为 *true*.


## 加载状态

如果 3D 模型相当大，渲染出来可能需要一段明显的时间，这段时间会因玩家的硬件和许多其他因素而异。有时你需要确保模型已完成加载后再执行其他操作。例如，如果你想把玩家传送到高空中的平台上，就需要先确保平台已完全渲染，再把玩家移动过去，否则玩家可能会直接穿过平台掉下去。

要检查 3D 模型是否已完成渲染，请查看实体的 `GltfContainerLoadingState` 组件。该组件为只读，且存在于任何同时具有 `GltfContainer`组件的实体时，组件不会被激活。

该组件只有一个名为 `currentState`的属性，其中保存来自 `LoadingState` 枚举的值。

以下示例使用一个系统定期检查实体 3D 模型的加载状态。如果状态为 `LoadingState.FINISHED`，你可能希望在此执行自定义逻辑并结束该系统的执行。

```ts
export function main() {
	const meshEntity = engine.addEntity()
	GltfContainer.create(meshEntity, { src: 'models/Monster.glb' })
	engine.addSystem((deltaTime) => {
		const loadingState = GltfContainerLoadingState.getOrNull(meshEntity)
		if (!loadingState) return
		switch (loadingState.currentState) {
			case LoadingState.LOADING:
				console.log('网格正在加载')
				break
			case LoadingState.FINISHED:
				console.log('网格已完成')
				// 执行自定义逻辑
				break
			case LoadingState.FINISHED_WITH_ERROR:
				console.log('网格已完成，但可能存在问题')
				break
			case LoadingState.UNKNOWN:
				console.log('网格处于未知状态')
				break
		}
	})
}
```

## 高级语法

创建一个 `MeshRenderer` 组件的完整语法（不使用任何辅助函数简化）如下：

```ts
MeshRenderer.create(myBox, {
	mesh: {
		$case: 'box',
		box: { uvs: [] },
	},
})

MeshRenderer.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: { uvs: [] },
	},
})

MeshRenderer.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshRenderer.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

基础协议就是这样解释 MeshRenderer 组件的。辅助函数将其抽象掉并提供更友好的语法，但在底层它们输出的就是这种语法。

该 `$case` 字段允许你指定一种允许的类型。每种类型都支持不同的一组参数。在上面的示例中， `立方体` 类型支持一个 `uvs` 字段中。

的支持值如下： `$case` 如下：

* `立方体`
* `平面`
* `球体`
* `圆柱体`

根据 `$case`的值，可以定义相应形状的对象，并传入任何相关属性。

若要向实体添加一个 `MeshRenderer` 组件，而该实体可能已经有该组件的实例，请使用 `MeshRenderer.createOrReplace()`。像 `MeshRenderer.setBox()` 这样的辅助函数会处理覆盖现有组件实例，但在一个已经具有该组件的实体上运行 `MeshRenderer.create()` 会返回错误。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
