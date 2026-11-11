> 通过名称或标签在代码中引用物品。

# 在代码中引用物品

你可以在代码中引用通过 Creator Hub 拖放界面添加的项目。这对于为这些项目添加复杂行为，或在代码的其他部分引用它们很有用。

## 按名称获取

在使用 Creator Hub 并通过将实体拖入画布来添加它们时，每个实体都有一个唯一名称。请使用 `engine.getEntityOrNullByName()` 函数来在代码中引用这些实体之一。

使用 `EntityNames` 枚举，方便访问你通过 Creator Hub 添加的实体名称，或者像在 Scene Editor 的场景实体树视图中显示的那样，将名称写成字符串。

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	// 使用 EntityNames 枚举
	const door1 = engine.getEntityOrNullByName(EntityNames.Door_1)

	// 将名称写成字符串
	const door2 = engine.getEntityOrNullByName('Door 2')

	// 确保场景中同时存在两扇门
	if (door1 && door2) {
		// 
	}

}
```

![](/docs/_remote/6c4b9156f30308b7eb4c3fc9.webp)

该 `EntityNames` 该枚举包含 Creator Hub 添加的全部实体列表，并会在你做出任何更改后立即更新。如果你导入 `EntityNames.` 到代码中，IDE 会向你提供一个下拉列表，其中包含所有可用的实体名称。

![](/docs/_remote/c76350566b1b5677df83aaca.webp)

你也可以使用 `engine.getEntityByName<EntityNames>()` 函数，并传入 `<EntityNames>` 作为 [TypeScript 泛型](https://www.typescriptlang.org/docs/handbook/2/generics.html)，用于验证该名称的实体确实存在于场景中。如果被引用的实体在 Creator Hub 中被重命名，此方法会向你发出错误警告。由于该函数的输出不能为 `null`，因此你可以省去检查实体是否存在。

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	const door1 = engine.getEntityByName<EntityNames>(EntityNames.Door_1)

	// 无需检查 null
	console.log(Transform.get(door1).position.x)

}
```


> [!WARNING]
> **📔 注意**：请确保你只在以下位置使用 `engine.getEntityOrNullByName()` 和 `engine.getEntityByName()` 在 `main()` 函数内、在以下函数执行之后运行的函数中，或者在系统中使用。 `main()`，或者在系统中使用。如果在这些上下文之外使用，Scene Editor 中创建的实体可能尚未实例化。


一旦你通过上述任一方法获取了实体的引用，就可以自由对其执行任何操作，例如添加或移除组件、修改现有组件的值，甚至将实体从引擎中移除。

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// 获取实体
	const door = engine.getEntityOrNullByName(EntityNames.Door_3)
	// 验证实体存在
	if (door) {
		// 添加指针事件回调
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open' },
			},
			function () {
				// 打开门
			}
		)
	}
}
```

通过 Scene Editor 添加的所有实体都有一个 `名称` 组件，你也可以像这样遍历它们：

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## 按标签获取

你也可以通过标签获取实体。标签是一种将实体分组的方式，可用于识别具有相同用途或行为的实体。

通过 **标签** 在项目属性面板顶部的部分为实体添加标签。你可以从通用标签中选择，例如 **标签组 1** 一直到 **标签组 4**，或者创建一个更具体名称的自定义标签。

![](/docs/_remote/feb4023234754c8ebd2b7590.webp)


> [!NOTE]
> **💡 提示**：一个实体可以分配多个标签。
> 
> <img src="/docs/_remote/22e3f2ff154ade672d73545c.webp" alt="" data-size="original">


然后，你可以使用 `engine.getEntitiesByTag()` 函数获取拥有特定标签的所有实体。这非常适合遍历具有相同用途或行为的一组实体。

```ts
import { engine } from '@dcl/sdk/ecs'

export function main() {
	const taggedEntities = engine.getEntitiesByTag('myTag')
  
	for (const entity of taggedEntities) {
      // 对每个实体做点什么
    }
}
```

你也可以在代码中为实体添加或移除标签。如果你想根据某些逻辑更改标签，或动态生成具有特定标签的实体，这会很有用。

```ts
import { Tags } from '@dcl/sdk/ecs'

Tags.remove(entity, tagName);
Tags.add(entity, tagName);
```

## 获取某个项目的所有子项

一旦你拥有某个特定项目的引用，就可以获取屏幕左侧实体树中作为其子项分组的所有项目。下面的脚本会获取父实体，然后遍历其每个带有 Transform 组件的子项。你可以在遍历每个子项时应用任何自定义逻辑。

```ts
import { engine, Entity, Transform, Name, getEntitiesWithParent } from '@dcl/sdk/ecs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// 获取父实体
	const parent = engine.getEntityByName<EntityNames>(EntityNames.ParentEntity)

	// 获取子项完整列表
	const children = getEntitiesWithParent(engine, parent)
	for (const child of children) {
   		// 处理每个子实体
	}
}
```

## 智能项目触发器

你可以检测智能项目的 **触发事件**，并使用自定义代码响应这些事件。例如，你可以放置一个按钮智能项目，并在点击按钮时触发自定义代码。

使用 `getTriggerEvents` 以获取一个可处理来自特定智能项目的触发事件的对象，然后使用返回对象的 `.on()` 函数订阅回调函数。每次触发事件发生时，都会执行此回调函数。

例如，如果场景中有一个按钮，并带有以下通用 **On Click** 事件，你就可以编写下面的代码，在按钮被激活时运行自定义代码。

![](/docs/_remote/ed2a86256c21aaf6084b2d47.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const restart = engine.getEntityOrNullByName(EntityNames.Restart_Button)
	if (restart) {
		const restart_event = getTriggerEvents(restart)
		restart_event.on(TriggerType.ON_CLICK, () => {
			// restartGame()
		})
	}
}
```

你也可以类似地订阅其他类型的触发事件，例如 **ON\_PLAYER\_ENTERS\_AREA**, **ON\_SPAWN**, **ON\_TWEEN\_END**等项目就是如此。

## 智能项目动作

你可以检测智能项目的 **动作**，并使用自定义代码响应这些事件。例如，你可以放置一个门智能项目，并在 **Open** 动作被调用时运行自定义代码。

使用 `getActionEvents` 以获取一个用于处理特定智能项目动作的对象。然后你可以使用 `.on()` 函数订阅回调函数。每次动作发生时，都会执行此回调函数，无论该动作是由另一个智能项目激活，还是由你自己的自定义代码激活。

例如，如果场景中有一扇门，并带有以下默认 **Open** 动作，你就可以编写下面的代码，在门被打开时运行自定义代码。

![](/docs/_remote/d8d14bd55320cbd4d130d66f.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (door) {
		// 检测动作
		const actions = getActionEvents(door)
		actions.on('Open', () => {
			console.log('门已打开！！')
			// 自定义代码
		})

		// 检测触发器
		const triggers = getTriggerEvents(door)
		triggers.on(TriggerType.ON_CLICK, () => {
			console.log('门被点击了！！')
			// 自定义代码
		})
	}
}
```

你也可以在代码中发出动作事件，这样就可以利用智能项目 Action 组件中已经定义好的动作。下面的代码片段会在按钮智能项目被触发时，调用门智能项目上的“Open”动作。

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const button = engine.getEntityOrNullByName(EntityNames.Red_Button)
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (button && door) {
		// 动作和触发器的引用
		const buttonTriggers = getTriggerEvents(button)
		const doorActions = getActionEvents(door)

		// 检测按钮上的触发器
		buttonTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
			// 打开门
			doorActions.emit('Open', {})
		})
	}
}
```


> [!NOTE]
> **💡 提示**：如果你不打算做非常复杂的事情，除了编写代码之外，你也可以创建一个自定义智能项目来处理你想执行的动作。参见 [让任意项目变成智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/).


## 其他智能项目组件

智能项目可以包含属于 asset-packs 库的特殊组件，例如 `状态` 或 `计数器`。这些组件不是 Decentraland SDK 的一部分，但可以通过库中的 `getComponents()` 函数获取。然后你可以在场景代码中读取或写入这些组件的值，使智能项目行为与代码之间的集成更加紧密。

下面的示例会在宝箱智能项目的动作被触发时，读取并记录其 State 组件的值。

```ts

import { engine } from '@dcl/sdk/ecs'
import { getComponents } from '@dcl/asset-packs'
import { getTriggerEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'


export function main() {

    const chest = engine.getEntityByName<EntityNames>(EntityNames.chest)
 
    if (chest) {

        const chestTriggers = getTriggerEvents(chest)

        chestTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
            const { States } = getComponents(engine)
            let state = States.getMutableOrNull(chest)?.currentValue
            console.log("宝箱新状态 ", state)
        })
    }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/reference-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
