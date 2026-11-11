> 如何为场景中的 3D 模型制作动画

# 3D 模型动画

中的 3D 模型 *.glTF* 和 *.glb* 格式中可以包含任意数量的动画。动画通过指定一系列 *关键帧* 在时间轴上排列的关键帧，网格随后会在一个姿势与另一个姿势之间进行混合，以模拟连续运动。

大多数 3D 模型动画都是 [*骨骼动画*](https://en.wikipedia.org/wiki/Skeletal_animation)。这些动画将模型的复杂几何简化为“火柴人”式骨架，把网格中的每个顶点连接到最近的 *骨骼* 在 *骨架*。建模师会把骨架调整成不同的姿势，网格则会拉伸和弯曲以跟随这些动作。

作为另一种方式， *顶点动画* 无需骨架即可为模型制作动画。这些动画直接指定模型中每个顶点的位置。Decentraland 也支持这类动画。

参见 [动画](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/animations.md) 了解如何为 3D 模型创建动画的详细信息。请阅读 [形状组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/) 以了解如何将 3D 模型导入场景。


> [!NOTE]
> **💡 提示**：动画通常更适合让某物在原地移动，而不是改变实体的位置。例如，你可以设置一个动画让角色的脚原地动作，但要改变实体的位置，最好使用 Transform 组件。请参见 [定位实体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/) 以了解更多详情。


## 检查 3D 模型是否包含动画

并非所有 *glTF* 文件都包含动画。要查看是否有可用动画，你可以执行以下操作：

* 如果使用 [VS Code](https://code.visualstudio.com/)（推荐），请安装 *GLTF Tools* 扩展，然后在那里查看 glTF 文件的内容。
* 打开 [Babylon Sandbox](https://sandbox.babylonjs.com/) 网站，并将 glTF 文件（以及任何 *.jpg* 或 *.bin* 依赖文件）拖到浏览器中。
* 打开 *.glTF* 文件，用文本编辑器打开并向下滚动，直到找到 *"animations":*.


> [!NOTE]
> **💡 提示**：在 *骨骼* 动画中，动画名称通常由骨架名称、下划线和动画名称组成。例如 `myArmature_animation1`.


## 自动播放

如果 3D 模型包含任何动画，默认行为是始终循环播放其中第一个动画。

要避免这种行为，请添加一个 `Animator` 组件到包含该模型的实体上，然后显式处理动画播放。如果实体中存在一个 `Animator` 组件，所有动画默认处于 `playing: false` 状态，并且需要手动播放。


> [!NOTE]
> **💡 提示**：在 [场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)中，你可以添加一个 **Animator** 组件。请参见 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components)。你也可以通过无代码方式控制动画，使用 **操作**，请参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).


## 显式处理动画

一个 `Animator` 组件用于访问实体的所有动画，也可用于显式告诉实体播放或停止某个动画。 `Animator` 组件包含一个 `states`数组，这个列表必须为 3D 模型可以执行的每个动画都包含一个对象。单个 `Animator` 可以包含所需数量的状态。

```ts
// 创建实体
const shark = engine.addEntity()

// 为其添加一个 3D 模型
GltfContainer.create(shark, {
	src: 'models/shark.glb',
})

Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
		},
	],
})
```

每个 `状态` 对象会跟踪动画当前是否正在播放。


> [!WARNING]
> **📔 注意**： `Animator` 组件必须通过以下方式导入
> 
> > `import { Animator } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。


## 获取一个动画

通过以下方式从 `Animator` 按名称获取一个剪辑，使用 `.Animator.getClip()` 函数。此函数会返回动画状态对象的可变版本。

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
```

`Animator.getClip` 需要以下参数：

* `entity`： `Animator` 你要查询的组件所属实体。
* `clipName`：你要获取的剪辑名称字符串。

`Animator.getClip` 会获取动画状态的可变版本，因此你可以自由修改此函数返回的值。

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
swimAnim.loop = false
```


> [!WARNING]
> **📔 注意**：如果你尝试使用 `Animator.getClip()` 去获取一个未列在 `Animator` 组件中的剪辑，它会抛出错误。请使用 `Animator.getClipOrNull()` 如果你更希望得到一个 `null` 响应，而不是错误。


## 播放动画

该 `.playing` 字段决定动画当前是否正在播放。请注意，同一个 3D 模型中可能同时播放多个动画。

使用 `Animator.playSingleAnimation()` 函数作用于一个 `AnimationState` 对象。

```ts
Animator.playSingleAnimation(sharkEntity, 'swim')
```

如果该实体当时正在播放其他任何动画， `Animator.playSingleAnimation` 会停止它们。

`Animator.playSingleAnimation` 需要以下参数：

* `entity`： `Animator` 你想要影响的组件。
* `clipName`：你要播放的剪辑名称字符串。
* `resetCursor`: *（可选）* 如果 *true*，它会从头开始播放动画，即使该动画之前已暂停。如果 *false*，它将从暂停的位置继续播放。默认值： *true*.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim', false)
```

下表总结了 `Animator.playSingleAnimation()` 在使用不同的 `resetCursor` 属性时的行为：

|              | `resetCursor` = *false* | `resetCursor` = *true* （默认） |
| ------------ | ----------------------- | --------------------------- |
| **当前正在播放**   | 没有效果。                   | 从头播放。                       |
| **已暂停**      | 从上次播放的帧继续。              | 从头播放。                       |
| **已完成（非循环）** | 从头播放。                   | 从头播放。                       |

## 循环动画

默认情况下，动画会以循环方式播放，并无限重复。

通过设置 `loop` 属性在 `状态` 对象。

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			loop: false,
		},
	],
})
```

如果 `loop` 被设为 *false*时，动画只播放一次然后停止，并停留在最后一帧的姿势上。

## 停止动画

要停止实体正在播放的所有动画，请使用 `Animator.stopAllAnimations()`.

```ts
Animator.stopAllAnimations(shark)
```

`Animator.stopAllAnimations` 需要以下参数：

* `entity`： `Animator` 你想要影响的组件。
* `resetCursor`: *（可选）* 如果 *true*，它会回到动画第一帧的姿势。如果 *false*，则会暂停并保持当前姿势。默认值： *true*.


> [!WARNING]
> **📔 注意**：当使用 `Animator.playSingleAnimation`播放动画时，此函数会在后台处理停止所有其他动画。在这种情况下，你无需显式停止其他动画。


当一个非循环动画播放完毕时，默认情况下 3D 模型会保持在最后的姿势。 `shouldReset` 属性控制停止的动画再次播放时会发生什么：如果 *true*，动画会恢复到其初始状态（第一帧；如果以负的 `speed`速度播放，则为最后一帧），每当它从停止变为播放时都会如此。如果 *false* （默认值），则会从原处继续播放。

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			shouldReset: true,
			loop: true,
		},
	],
})
```

你也可以使用 `Animator.stopAllAnimations()` 随时显式将姿势重置回动画的第一帧。


> [!WARNING]
> **📔 注意**：重置姿势是一个突兀的变化。如果你想让模型平滑过渡到另一种姿势，请播放另一个动画，并通过逐渐调整它们的 `权重` 属性来进行混合。请参见 [动画权重](#animation-weight).


## 检测动画何时结束

当一个非循环动画播放结束时，引擎会将该动画状态的 `playing` 属性重置为 *false*。你的场景代码可以读取这个值来判断动画何时结束，例如在其后立即串接另一个动画。

```ts
let wasPlaying = false

engine.addSystem(() => {
	const animator = Animator.get(shark)
	const biteState = animator.states.find((state) => state.clip === 'bite')
	const isPlaying = biteState?.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('bite animation finished')
		// 串接下一个动画
		Animator.playSingleAnimation(shark, 'swim')
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 注意**：当每一帧轮询动画状态时，请始终通过 `Animator.get()` （只读）。不要使用 `Animator.getClip()` 或 `Animator.getMutable()` 来轮询：它们返回的是组件的可变版本，会在每一帧将其标记为已更改，并导致不必要的同步开销。
> 
> 该 `playing` 属性只有在动画自行结束时才会被引擎切换。循环动画会一直播放直到被停止，因此它们不会自动切换该属性，而 `speed` 设置为 0 的动画永远不会结束。



> [!WARNING]
> **📔 注意**：此功能仅受桌面客户端支持。


## 处理多个动画

如果一个 3D 模型内打包了多个动画，单个 `Animator` 组件即可处理它们全部。

```ts
// 创建实体
const shark = engine.addEntity()

// 为其添加一个 3D 模型
GltfContainer.create(shark, {
	src: 'models/shark.glb'
})

Animator.create(shark, {
	states:[{
			clip: "swim",
			playing: true,
			loop: true
		}, {
			clip: "bite",
			playing: true,
			loop: true
		}
	]
})
```

在上面的示例中，两个动画由不同的 `状态` 对象处理，然后它们都被分配到同一个 `Animator` 组件，则抛出错误。

在动画中，每根骨骼同一时间只能受一个动画影响，除非这些动画拥有一个 `权重` 使其总和值不超过 1 的

如果一个动画只影响角色的腿，而另一个只影响角色的头部，那么它们可以同时播放而没有问题。但如果它们都影响角色的腿，那么你必须要么一次只播放一个，要么以更低的 `权重` 权重值播放。

如果在上面的例子中， `bite` 动画只影响鲨鱼的嘴部，而 `swim` 动画只影响鲨鱼脊柱的骨骼，那么它们就可以同时播放。


> [!WARNING]
> **📔 注意**: `Animator.playSingleAnimation()` 会停止实体当前正在播放的所有其他动画。要同时播放多个动画，请手动修改 `playing` 动画状态中的属性。


## 动画速度

通过更改 `speed` 属性来改变动画播放速度。速度的默认值为 1。

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			speed: 2,
		},
	],
})
```

将速度设为小于 1 可让其播放更慢，例如设为 0.5 表示以一半速度播放。将其设为大于 1 可让其播放更快，例如设为 2 表示以两倍速度播放。

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.speed = 0.5
```

## 动画权重

该 `权重` 属性允许单个模型同时执行多个动画，并计算动画中所有动作的加权平均值。 `权重` 的值决定该动画在平均值中所占的重要性。

默认情况下， `权重` 等于 *1*。 `权重` 不能高于 *1*.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			weight: 0.2,
		},
	],
})
```

该 `权重` 实体中所有正在生效的动画的值应始终加起来等于 1。如果总和小于 1，加权平均在剩余部分的计算中将使用骨架的默认姿势。

例如，在上面的代码示例中，我们正在播放 *swim* 动画，它只有一个 `权重` 为 *0.2*。

该 `权重` 属性可以以有趣的方式使用，例如 `权重` 的 *swim* 可以按鲨鱼游动速度的比例设置，这样你就不需要为快游和慢游分别创建多个动画。

你也可以在 `权重` 开始和停止动画时逐渐改变该值，以实现更自然的过渡，并避免从默认姿势突然跳到动画的第一姿势。


> [!WARNING]
> **📔 注意**：所有 `权重` 作用于 3D 模型骨骼的动画值之和不能超过 1。如果多个动画同时影响同一组骨骼，它们的权重值总和必须小于 1。


```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.weight = 0.5
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/3d-model-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
