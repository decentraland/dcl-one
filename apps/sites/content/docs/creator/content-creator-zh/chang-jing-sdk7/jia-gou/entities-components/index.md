> 了解 Decentraland 场景中实体和组件的基础知识

# 实体与组件

Decentraland 场景围绕 [*实体*, *组件* 和 *系统*](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system)。这是多种游戏引擎架构中常见的一种模式，可轻松实现组合性和可扩展性。

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

## 概述

*实体* 是在 Decentraland 场景中构建一切的基本单元。场景中所有可见和不可见的 3D 对象以及音频播放器都会各自作为一个实体。实体不过是一个 id，可由组件引用。实体本身没有任何自己的属性或方法，它只是用于将多个组件组合在一起。

*组件* 定义实体的特性。例如，一个 `Transform` 组件会存储实体的坐标、旋转和缩放。一个 `MeshRenderer` 组件在场景渲染时会赋予实体一个可见形状（如立方体或球体），一个 `材质` 组件会给实体添加颜色或纹理。你也可以创建自定义组件来提供场景所需的数据，例如一个自定义的 `生命值` 可存储实体剩余的生命值，并将其添加到游戏中代表非玩家敌人的实体上。

如果你熟悉 Web 开发，可以将实体看作相当于 *元素* 包裹在 *DOM* 树，而组件则相当于 *属性* 中这些元素的属性。

在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)，你可以通过选择实体来查看属于该实体的组件。

![](/docs/_remote/f49a93029e97b3d8ef464fb3.webp)


> [!WARNING]
> **📔 注意**：在 SDK 的早期版本中，实体是 *对象* ，这些对象会被实例化，并且可以通过扩展来添加函数。从 SDK 7.0 版本开始，实体只是一 个 ID。这种结构更符合 [面向数据编程](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/) ，并有助于提升场景性能。


![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

像 `Transform`, `材质` 或任何 *形状* 组件与场景渲染密切相关。如果这些组件中的值发生变化，仅此就足以让引擎在下一帧改变场景的渲染方式。

引擎是场景中位于中间并管理所有其他部分的那一部分。它决定渲染哪些实体，以及玩家如何与它们交互。它还协调来自 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 的函数何时执行。

组件用于存储其所引用实体的数据。它们只能存储这些数据，不能自行修改这些数据。组件值的所有更改都由 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/)执行。系统与组件和实体本身完全解耦。实体和组件并不关心 *系统* 正在对它们进行操作。

## 实体和组件的语法

下面的示例展示了声明和配置基本实体与组件的一些基础操作。

```ts
export function main() {
	// 创建一个实体
	const door = engine.addEntity()

	// 通过 Transform 组件给实体设置位置
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// 通过 GltfContainer 组件给实体设置可见形状
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```


> [!WARNING]
> **📔 注意**：在 SDK 的早期版本中，必须手动将实体添加到引擎中才能开始渲染。从 SDK 7 版本开始，一旦实体被分配了组件，就会隐式添加到引擎中。


创建组件时，它总是会被分配给一个父实体。随后组件的值会影响该实体。


> [!NOTE]
> **💡 提示**：你无需逐个创建实体，可以一次性从一个 [组合体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites/) 文件。


## 移除实体

要从引擎中移除实体，请使用 `engine.removeEntity()`。此函数会返回一个 `布尔值`: `true` ，如果实体已被移除则返回 `false` ，如果移除被拒绝则返回。

```ts
export function main() {
	// 创建一个实体
	const door = engine.addEntity()

	// 通过 GltfContainer 组件给实体设置可见形状
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})

	// 移除实体
	const removed = engine.removeEntity(door)
	console.log('实体已移除：', removed) // true
}
```

如果被移除的实体有任何子实体，这些子实体的父级会改回默认的 `engine.RootEntity` 实体，该实体位于场景基准位置，缩放为 *1*.

### 渲染器保留的实体

某些实体 id 被渲染器保留用于远程玩家头像。你无法移除这些实体。如果你调用 `engine.removeEntity()` 到渲染器保留的实体上，它会返回 `false` ，并保持所有组件不变。

该 [命名的保留实体](#reserved-entities) (`engine.RootEntity`, `engine.PlayerEntity`, `engine.CameraEntity`）是一个特例： `engine.removeEntity()` 仍然返回 `false` ，但它们的组件 **都是** 会被清除。这意味着你可以移除其中属于你自己的组件（例如移除一个 `InputModifier` 从 `engine.PlayerEntity`），即使实体 id 本身永远不会被释放。

```ts
// 尝试移除一个保留实体
const result = engine.removeEntity(engine.PlayerEntity)
console.log(result) // false — 实体 id 不会被释放
// 但 PlayerEntity 上场景自身的组件会被清除
```

你可以在移除实体时检查返回值，以处理边界情况：

```ts
if (!engine.removeEntity(someEntity)) {
	console.log('实体无法移除（保留）')
}
```

### 移除实体及其子实体

要移除一个实体以及它的所有子实体（以及其子实体的子实体，递归地），请使用 `removeEntityWithChildren()` 辅助函数。

```ts
export function main() {
	// 创建父实体
	const door = engine.addEntity()

	// 创建子实体
	const doorKnob = engine.addEntity()

	// 给这些实体设置可见形状
	GltfContainer.create(door, {
		src: 'models/door.glb',
	})
	GltfContainer.create(doorKnob, {
		src: 'models/doorKnob.glb',
	})

	// 父级
	Transform.create(doorKnob, {
		parent: door,
	})

	// 同时移除父级和子级
	removeEntityWithChildren(engine, door)
}
```


> [!WARNING]
> **注意：** 如果树中的任何位置存在渲染器保留的实体， `removeEntityWithChildren` 它会移除所有其他后代，但保留该保留实体。该保留实体的 `Transform.parent` 将指向一个已移除的实体。只有当你的场景将一个保留实体设为某个场景实体的子级时才会出现这种情况，这并不常见。



> [!NOTE]
> **💡 提示**：在某些情况下，与其从引擎中移除实体，不如将其设为不可见，以便之后能够毫无延迟地再次加载。参见 [设为不可见](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/#make-invisible)


### 在幕后移除实体

实体只是一个由其组件引用的 id。因此，当你移除实体时，实际上是在移除引用该实体的每个组件。如果你手动移除实体的所有组件，对玩家来说看起来与执行 `engine.removeEntity()`。不过， `engine.removeEntity()` 还会执行一些额外的内部记账操作，将实体 id 标记为不再使用，因此它始终是移除实体的推荐方式。

## 嵌套实体

一个实体可以拥有其他实体作为子级。借此，我们可以像网页的 HTML 一样，将实体组织成树状结构。

![](/docs/_remote/28a71287061591ece4e52409.webp)

要将一个实体设置为另一个实体的父级，子实体必须具有一个 `Transform` 组件。然后你可以设置 `parent` 字段，并将父实体的引用写入其中。

```ts
export function main() {
	// 创建实体
	const parentEntity = engine.addEntity()

	const childEntity = engine.addEntity()

	// 设置父级
	Transform.create(childEntity, {
		parent: parentEntity,
	})
}
```

一旦分配了父级，就可以从子实体的 `parent` 字段中读取它的 `Transform` 组件，则抛出错误。

```ts
// 从实体获取父级
const parent = Transform.get(childEntity).parent
```

如果父实体具有一个 `Transform` 会影响其位置、缩放或旋转的组件，那么它的子实体也会受到影响。所有位置或旋转值都会相加，所有缩放值都会相乘。

如果父实体或子实体中任意一个没有 `Transform` 组件，则使用以下默认值。

* 对于 **position**，父级的中心是 *0, 0, 0*
* 对于 **旋转** ，父级的旋转是四元数 *0, 0, 0, 1* （等同于欧拉角 *0, 0, 0*)
* 对于 **缩放**，父级被视为具有大小 *1*。父级的任何缩放都会按比例影响子级的位置和缩放。

没有形状组件的实体在场景中是不可见的。这些实体可用作包装器，以组的形式处理和定位多个实体。

要将子实体与其父实体分离，你可以将该实体的父级设为 `engine.RootEntity`.

```ts
const mutableChildTransform = Transform.getMutable(childEntity)
mutableChildTransform.parent = engine.RootEntity
```


> [!WARNING]
> **📔 注意**：在处理与其他玩家同步的嵌套实体时，请使用 `parentEntity()` 函数，而不是 `parent` 中的实体。参见 [有父子关系的实体](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/#parented-entities)


在场景编辑器中，你可以在左侧面板看到场景中嵌套实体的整个层级结构。

![](/docs/_remote/87863329d94332c21873d9dd.webp)

## 通过 ID 获取实体

场景中的每个实体都有一个唯一编号 *id*。你可以根据这个 ID 从引擎中检索引用特定实体的组件。

```typescript
// 获取一个 Transform 组件
Transform.get(1000 as Entity)
```


> [!WARNING]
> **📔 注意**：介于 *0* 和 *511* 之间的实体 id 被引擎保留给固定实体，例如玩家头像、基础场景等。


例如，如果玩家的点击或某个 [射线检测](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting/) 击中了某个实体，这将返回被击中实体的 id，你可以使用上面的命令获取与该 id 匹配的实体的 Transform 组件。你也可以用同样的方式获取该实体的任何其他组件。

## 通过名称获取实体

在场景编辑器中通过拖放添加实体时，每个实体都有一个唯一名称。请使用 `engine.getEntityOrNullByName()` 函数在代码中引用这些实体之一。将实体名称作为字符串传入，名称应与场景编辑器界面左侧树视图中显示的一致。

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```


> [!WARNING]
> **📔 注意**：请确保你只在 `engine.getEntityOrNullByName()` 内部调用，或者在一个运行于 `main()` 函数中使用它，或者在 `main()`之后运行的函数中，或者在系统中使用。如果在这些上下文之外使用，场景编辑器 UI 中创建的实体可能还未实例化。


你可以自由地对通过此方法获取的实体执行任何操作，例如添加或移除组件、修改现有组件的值，或将实体从引擎中移除。

```ts
function main() {
	// 获取实体
	const door = engine.getEntityOrNullByName('door-3')
	// 验证实体是否存在
	if (door) {
		// 添加一个指针事件回调
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: '打开' },
			},
			function () {
				// 打开门
			}
		)
	}
}
```

通过场景编辑器 UI 添加的所有实体都具有一个 `名称` 组件，你可以像这样遍历它们：

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## 添加或替换组件

每个实体只能拥有某一类型的一个组件。例如，如果你尝试给一个已经有 Transform 的实体再次赋值，就会导致错误。

为避免此错误，你可以使用 `.createOrReplace` 而不是 `.create`。如果同类组件已存在，此命令会覆盖它们，否则就像 `.create`.

```ts
Transform.createOrReplace(door, {
	position: Vector3.create(5, 1, 5),
})
```


> [!WARNING]
> **📔 注意**：由于 `.createOrReplace` 在创建组件前会额外执行一次检查，因此使用 `.create`总是更高效。如果你确定实体还没有你要添加的那种组件，请使用 `.create`.


## 从实体访问组件

你可以通过使用实体的 `.get()` 或 `getMutable()` 函数来创建颜色。

```ts
export function main() {
	// 创建实体
	const box = engine.addEntity()

	// 创建并将组件添加到该实体
	Transform.create(box)

	// 获取组件的只读版本
	let transform = Transform.get(box)

	// 获取组件的可变版本
	let transform = Transform.getMutable(box)
}
```

该 `get()` 函数会获取该组件的只读引用。你无法通过该组件引用更改任何值。

如果你希望更改组件的值，请使用 `getMutable()` 函数。如果你更改组件可变版本中的值，就会直接影响该组件所属的实体。

参见 [可变数据](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/mutable-data/) 以了解更多详情。


> [!WARNING]
> **📔 注意**：仅在 `getMutable()` 你确实要更改组件值时才使用。如果不是，请始终使用 `get()`。这种做法遵循 [面向数据编程](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/)原则，并能显著提升场景性能。


```ts
// 获取组件的可变版本
let transform = Transform.getMutable(box)

// 更改组件的一个值
transform.scale.x = 5
```

上面的示例直接修改了 *x* Transform 组件上的 scale 值。

如果你不完全确定实体是否具有你要检索的组件，请使用 `getOrNull()` 或 `getMutableOrNull()`.


> [!WARNING]
> **📔 注意**：尽量避免使用 `getOrNull()` 或 `getMutableOrNull()` ，因为这些函数涉及额外检查，因此效率低于 `.get()` 和 `getMutable()`.


```ts
//  getOrNull
const transformOrNull = Transform.getOrNull(myEntity)

//  getMutableOrNull
const mutableTransformOrNull = Transform.getMutableOrNull(myEntity)
```

如果你要检索的组件在实体中不存在：

* `get()` 和 `getMutable()` 会返回错误。
* `getOrNull()` 和 `getMutableOrNull()` 返回 `空值`.

## 从实体移除组件

要从实体中移除组件，请使用实体的 `deleteFrom()` 方法。

```ts
Transform.deleteFrom(myEntity)
```

如果你尝试移除实体中不存在的组件，此操作不会引发任何错误。


> [!WARNING]
> **📔 注意**：要一次性移除实体的所有组件，请参见 [本节](#remove-entities)


## 检查组件

你可以使用 `has()` 函数来检查实体是否拥有某个组件的实例。该函数返回 *true* 如果组件存在则返回 *false* ，如果不存在则返回。這在场景中的条件逻辑里非常有用。

```ts
const hasTransform = Transform.has(myEntity)
```


> [!NOTE]
> **💡 提示**：你还可以 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/) 来获取持有某个特定组件或某组特定组件的组件完整列表。不要手动遍历场景中的所有实体并逐个使用 `has()`进行检查，那种方法效率要低得多。


## 检查组件的变化

使用 `onChange` 函数，可在给定实体的组件值发生变化时运行回调函数。它适用于任何组件，是帮助保持代码可读性的绝佳捷径。

回调函数可以包含一个输入参数，其中包含组件的新状态。

```ts
Transform.onChange(cubeEntity, (newTransform) => {
	if (!newTransform) return
	console.log(
		'立方体位置已更改：',
		newTransform.position,
		newTransform.rotation
	)
})

VisibilityComponent.onChange(cubeEntity, (newVisibilityComponent) => {
	if (!newVisibilityComponent) return
	console.log('立方体可见性已更改：', newVisibilityComponent.visible)
})
```

如果该组件从实体中移除，则函数调用时的输入为 `undefined`.


> [!NOTE]
> **💡 提示**： `.onChange()` 函数既适用于 SDK 的原生组件，也适用于 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 由创作者定义的组件。


## 获取子实体

要访问某个父实体的所有直接子实体，请使用 `getEntitiesWithParent`。它的参数为 `引擎` 以及 `parent` 实体，并返回所有以该实体为父级的实体列表。请注意，它只返回直接子实体，不返回子实体的子实体。

```ts
import { getEntitiesWithParent } from '@dcl/sdk/ecs'

const children = getEntitiesWithParent(engine, myEntity)
for (const child of children) {
   // 处理每个子实体
}
```

如果你想访问一个实体的所有后代，不管它们嵌套了多深，请使用函数 `getComponentEntityTree()`。该函数不会手动逐层遍历层级，而是返回一个所有后代的扁平列表，便于迭代。它还会筛选，只返回具有给定组件或组件列表的实体。

```ts
import { getComponentEntityTree } from '@dcl/sdk/ecs'

export function main() {
	// 创建一个带有嵌套子实体的父实体
	const parentEntity = engine.addEntity()
	Transform.create(parentEntity, {
		position: Vector3.create(8, 0, 8),
	})

	// ……假设父实体有多个子实体和孙实体

	// 遍历父实体的所有后代
	for (const descendantEntity of getComponentEntityTree(
		engine,
		parentEntity,
		Transform
	)) {
		// 访问每个后代实体
		const transform = Transform.get(descendantEntity)
		console.log('后代位置：', transform.position)
	}
}
```

该 `getComponentEntityTree` 函数接受三个参数：

* `引擎`：运行这些实体的引擎实例
* `entity`：要从其开始的根实体
* `组件`：用于筛选的组件（通常为 `Transform` 用于空间层级）

该函数返回一个生成器，会逐个产出树结构中的每个后代实体。结果中只会包含具有指定组件的实体。

你可以将其与其他组件检查结合使用，以在层级中查找特定实体：

```ts
// 查找具有特定名称的所有后代
for (const descendantEntity of getComponentEntityTree(
	engine,
	parentEntity,
	Transform
)) {
	const name = Name.getOrNull(descendantEntity)
	if (name && name.value === 'targetEntity') {
		console.log('已找到目标实体：', descendantEntity)
	}
}
```

## 保留实体

某些实体 id 被保留给每个场景中都存在的特殊实体。可以通过以下别名访问它们：

* `engine.RootEntity`
* `engine.PlayerEntity`
* `engine.CameraEntity`


> [!WARNING]
> **📔 注意**：避免在它们初始化之前引用这些实体。为避免此问题，请在 `main()` 函数中，或在系统中引用这些实体。


## 根实体

场景中的所有实体都是 `engine.RootEntity`的子级，直接或间接皆然。

这个实体没有 Transform 组件，但它用于处理一些表示更全局设置的组件，例如 [天空盒控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control/), [光标位置](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#check-the-players-cursor-position)，或 [屏幕尺寸](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui-positioning/#responsive-ui-size).

## 玩家实体

该 `engine.PlayerEntity` 实体代表玩家的头像。

获取玩家的 `Transform` 组件以获取玩家当前的位置和旋转，参见 [用户数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/)。玩家的 Transform 是只读的，要修改它请使用 `movePlayerTo()` 函数， [了解更多](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/move-player/).

你也可以通过将对象设置为该实体的子级来将它们附加到玩家身上，不过 [附加到玩家](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#attach-an-entity-to-an-avatar) 通常是更好的选择。

## 摄像机实体

该 `engine.CameraEntity` 实体代表玩家的摄像机。

获取摄像机的 `Transform` 组件以获取摄像机的位置和旋转。该实体的 Transform 也是只读的。要修改摄像机角度或位置，请使用一个 [虚拟摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#using-virtual-cameras).

你也可以获取摄像机的 `CameraMode` 组件，以了解玩家是否正在使用第一人称或第三人称摄像机模式，参见 [摄像机模式](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#check-the-players-camera-mode).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
