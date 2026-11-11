> 了解如何处理来自组件的只读数据和可变数据

# 可变数据

当引用来自某个 [组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/)，你可以获取可变版本或只读（不可变）版本。

在可能的情况下，你应该始终使用只读版本的数据。与始终处理同一份数据的可变版本相比，这种做法可以显著提升场景性能。

该 `.get()` 组件中的函数会返回该组件的只读（不可变）版本。你只能读取其值，不能更改其中的任何属性。

该 `.getMutable()` 函数会返回组件的一个版本，使你能够更改其值。只有在你计划对组件进行修改时才使用可变版本，否则始终使用 `get()`.

```ts
// 获取只读（不可变）版本
const immutableTransform = Transform.get(myEntity)

// 以下写法不起作用：
// immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// 下面这一行确实会更改实体的位置
mutableTransform.position.y = 2
```

一个好的做法是遍历只读组件来检查值，并且仅在需要更改时才获取某个单独组件的可变版本。

```ts
// 硬编码的最大高度
const MAX_HEIGHT = 10

// 定义系统
function HeightLimitSystem(dt: number) {
	// 遍历所有具有 Transform 组件的实体
	for (const [entity] of engine.getEntitiesWith(Transform)) {
		// 获取只读值
		const currentHeight = Transform.get(entity).position.y

		// 比较数值
		if (currentHeight > MAX_HEIGHT) {
			// 获取可变版本以进行更改
			const mutableTransform = Transform.getMutable(entity)

			// 更改变换
			mutableTransform.position.y = MAX_HEIGHT
		}
	}
}

// 将系统添加到引擎中
engine.addSystem(HeightLimitSystem)
```

在上面的示例中，系统会检查某个实体的只读 `Transform` 组件。每一帧都会检查位置的 *y* 是否高于硬编码的最大高度。如果变换上的高度恰好高于该限制，那么也只有在这时我们才会获取 Transform 的可变版本。这看起来可能会给场景增加额外工作量，但在一个每一帧都要检查游戏循环中的值、而且只偶尔进行更改的场景中，它会带来巨大的性能提升。

这种做法遵循了以下原则： [面向数据编程](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/)。由于它带来的巨大改进，这一做法也正逐渐被游戏行业采纳为行业标准实践。


> [!WARNING]
> **📔 注意**：在旧版 SDK（6.x 或更早版本）中，组件始终被视为可变的。这种模式也许更容易上手，但运行效率要低得多。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/mutable-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
