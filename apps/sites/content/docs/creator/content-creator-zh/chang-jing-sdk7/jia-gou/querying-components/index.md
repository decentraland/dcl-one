> 了解如何获取具有共同组件的实体列表，以便更轻松地检查或更新它们。

# 查询组件

你可以 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/) 使用方法 `engine.getEntitiesWith(...components)` 以跟踪场景中所有具有特定组件的实体。

[系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 通常会遍历这些查询中的实体，并对每个实体执行相同的操作。预先定义一组有效实体是节省资源的好方法，尤其适用于在游戏循环的每个 tick 中运行的函数。如果在每个 tick 中，你的系统都必须遍历场景中的每一个实体来寻找它需要的实体，那将会非常低效。

你可以按如下方式访问查询中的实体。

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	//...
}
```

## 必需组件

在进行查询时，指定添加到该组中的每个实体都需要包含哪些组件。你可以列出任意多个组件，查询只会返回具有 **全部** 所列组件的实体。

```ts
for (const [entity] of engine.getEntitiesWith(
	Transform,
	Physics,
	NextPosition
)) {
	//...
}
```


> [!NOTE]
> **💡 提示**：如果你的查询返回了你不需要处理的实体，可以考虑创建一个自定义组件，将其作为 [标记](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/#components-as-flags)。这个组件不需要包含任何属性，但可以用来标记你可能希望区别对待的某个特定实体子组。


## 在系统中使用查询

```ts
// 定义一个系统
function PhysicsSystem(dt: number) {
	// 查询同时包含 Transform 和 Physics 组件的实体
	for (const [entity] of engine.getEntitiesWith(Transform, Physics)) {
		const transform = Transform.getMutable(entity)
		const vel = Physics.get(entity).velocity
		transform.position.x += vel.x
		transform.position.y += vel.y
		transform.position.z += vel.z
	}
}

// 将系统添加到引擎
engine.addSystem(PhysicsSystem)
```

在上面的示例中， `PhysicsSystem` 函数会遍历查询中的实体，并在游戏循环的每个 tick 中执行。

* 如果场景中有多个 *球* 实体，每个实体都带有一个 `变换` 和一个 `Physics` 组件，那么它们将会被处理，并且它们的位置会在每个 tick 中更新。
* 如果你的场景中还有其他实体，例如一个 *篮筐* 和一个 *记分板* 它们只具有一个 `变换` 但没有一个 `Physics` 组件，那么它们就不会受到 `PhysicsSystem`.

## 处理实体和组件

该 `getEntitiesWith` 函数返回一个集合，其中包含对一组实体的引用，并且也可以选择性地包含对所列组件的引用。

使用最简单的语法时，你只会获取到对应实体的引用列表。

```ts
for (const [entity] of engine.getEntitiesWith(myComponent, myOtherComponent)) {
	//...
}
```

在遍历这个实体列表时，你可以通过使用 `.get` 或 `getMutable`.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	// 获取只读版本
	const transformReadOnly = Transform.get(entity)

	// 获取可变版本
	const transformMutable = Transform.getMutable(entity)
}
```

你也可以选择直接将查询返回的集合中的每个已列出组件的引用一起获取。为此，只需把多个引用一起声明，每个你想获取的组件对应一个引用。添加这些引用是可选的，而且你也不需要声明对 *全部* 查询中组件的引用。

```ts
// 返回实体和第一个列出的组件的引用
for (const [entity, component1] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// 遍历实体列表
}

// 返回实体和前两个列出的组件的引用
for (const [entity, component1, component2] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// 遍历实体列表
}
```


> [!WARNING]
> **📔 注意**：这些引用是只读的。要获取这些组件的可变版本，你需要使用 `.getMutable` 函数并引用实体。


然后，在你遍历结果集合时，就可以引用这些内容；在每个条目中，你都可以访问实体及其对应的组件引用。

```ts
for (const [entity, transformReadOnly] of engine.getEntitiesWith(Transform)) {
	console.log('entity id: ', entity)
	console.log('has position : ', transformReadOnly.position)
}
```

## 订阅变化

一个常见的用法是在某个组件的数据发生变化时才运行一个函数。使用 [OnChange](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/subscribe-to-changes/) 函数，这样就无需定义一个系统，也无需显式比较旧值和新值。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
