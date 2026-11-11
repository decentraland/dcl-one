> 检测组件中的变化，并在每次变化时运行函数

# 订阅变更

一种简洁的编写代码的方式是订阅事件，并在事件发生时运行一个函数。

一些 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/) 作为 SDK 的一部分预先定义好了，但你也可以使用 `onChange()` 方法在任何组件上实现相同效果。它也适用于任何 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 你定义的组件，而无需做任何额外工作。

例如，下面的函数会检查 `AvatarEquippedData` 玩家实体上的组件，并在玩家更换任何已装备的穿戴物或表情时运行一个函数。组件的新值会作为函数参数传入。

```ts
import { AvatarEquippedData } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('新的佩戴物列表：', equipped.wearableUrns)
		console.log('新的表情列表：', equipped.emoteUrns)
	})
}
```

得益于 `onChange()` 方法，无需创建一个系统并在每一帧中反复检查新值，它极大地简化了这个非常常见的用例。


> [!WARNING]
> **📔 注意**：不要在 `onChange()` System 中使用，因为那会在游戏循环的每一帧都订阅一次该函数的新副本，并且可能导致崩溃。


同样的方法也可直接用于 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/)。例如：

```ts
// 定义组件
export const MyComponent = engine.defineComponent('myComponent', {
	value1: Schemas.Boolean,
	value2: Schemas.Float,
})

// 用法
export function main() {
	// 创建实体
	const myEntity = engine.addEntity()

	// 创建组件实例
	MyComponent.create(myEntity, {
		value1: true,
		value2: 10,
	})

	// 订阅变更
	MyComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.value1)
		console.log(componentData.value2)
	})
}
```

你也可以将此方法与 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/)结合使用，从而为场景中每个具有特定组件的实体批量订阅其各自的函数。

```ts
export function main() {
	for (const [entity] of engine.getEntitiesWith(MyComponent)) {
		MyComponent.onChange(entity, (componentData) => {
			if (!componentData) return
			console.log(componentData.value1)
			console.log(componentData.value2)
		})
	}
}
```

请注意，此方法只会订阅 `onChange()` 场景开始时就已存在的实体，例如通过该界面的 UI 创建的实体。 [创作者中心](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/).


> [!NOTE]
> **💡 提示**：如果你更希望处理那些不一定与组件变化相关的事件，我们建议导入 TypeScript 库 [Mitt](https://www.npmjs.com/package/mitt) 到你的场景中。该库提供了用于触发和监听事件的简单函数。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/subscribe-to-changes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
