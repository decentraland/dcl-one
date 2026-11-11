> 创建一个自定义组件来处理与实体相关的特定数据

# 自定义组件

实体的数据存储在其 [组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/)。Decentraland SDK 提供了一系列基础组件，用于管理实体的不同方面，例如位置、形状、材质等。引擎知道如何解析其中的信息，并会在这些值发生变化时立即相应地改变实体的渲染方式。

如果你的场景逻辑需要存储有关某个实体的信息，而这些信息不由 SDK 的默认组件处理，那么你可以在场景中创建一种自定义组件类型。然后你可以构建 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 来检查这些组件的变化并做出相应响应。

## 关于定义组件

要定义一个新组件，请使用 `engine.defineComponent`。每个组件都需要以下内容：

* 一个 **componentName**：一个唯一的字符串标识符，SDK 会在内部用它来识别此组件类型。它可以是任何字符串，只要保证唯一即可。
* 一个 **schema**：一个定义该组件所持有数据结构的类。
* **默认值** *（可选）*：一个包含默认值的对象；当未提供这些值时，用于初始化该组件的副本。

```ts
export const WheelSpinComponent = engine.defineComponent('wheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```


> [!WARNING]
> **📔 注意**：自定义组件必须始终编写在 `main()` 函数之外，放在单独的文件中。它们需要在 `main()` 执行。推荐将其放在 `/components` 文件夹中，位于 `/src`，并且每个组件各自放在单独的文件中。这样在未来项目中复用它们会更容易。


一旦你定义了自定义组件，就可以创建该组件的实例，用它来引用场景中的实体。创建组件实例时，需要为组件 schema 中的每个字段提供值。这些值必须符合各字段声明的类型。

```ts
// 创建实体
const wheel1 = engine.addEntity()
const wheel2 = engine.addEntity()

// 创建组件实例
WheelSpinComponent.create(wheel1, {
	spinning: true,
	speed: 10,
})

WheelSpinComponent.create(wheel2, {
	spinning: false,
	speed: 0,
})
```

每个添加了该组件的实体，都会实例化该组件的一个新副本，并保存该实体的特定数据。

你的自定义组件也可以执行其他组件上可用的常见功能：

```ts
// 从实体获取该组件的只读实例
const readOnlyInstance = MyCustomComponent.get(myEntity)

// 从实体获取该组件的可变实例
const mutableInstance = MyCustomComponent.getMutable(myEntity)

// 删除实体上的该组件实例
MyCustomComponent.deleteFrom(myEntity)
```

## 关于 componentName

每个组件都必须有一个唯一的组件名称或标识符，用于在内部区分它。你在代码的其他地方不需要使用这个内部标识符。一个好的做法是使用你为组件指定的同名名称，但首字母改为小写；不过真正重要的是，这个标识符在项目中必须唯一。

当创建要作为库的一部分共享的组件时，要注意你的库中的组件名称不能与使用该库的项目中任何组件名称重复，也不能与该项目所使用的其他库中的组件名称重复。为避免任何重名风险，推荐的最佳实践是在 `componentName` 字符串中包含库的名称。你可以遵循以下公式： `${packageName}::${componentName}`。例如，如果你构建了一个`MyUtilities` 库，其中包含一个 `MoveEntity` 组件，则将该组件的 `componentName` 设为 `MyUtilities::moveEntity`.

## 将组件作为标记

你可能希望添加一个只用来将实体与其他实体区分开的组件，而不使用它来存储任何数据。为此，请将 schema 留空对象。

这在使用 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/)时尤其有用。一个简单的标记组件可用于将实体与其他实体区分开，并避免系统遍历比需要更多的实体。

```ts
export const IsEnemyFlag = engine.defineComponent('isEnemyFlag', {})
```

然后你可以创建一个系统，遍历所有带有此组件的实体。

```ts
export function handleEnemies() {
	for (const [entity] of engine.getEntitiesWith(IsEnemyFlag)) {
		// 对每个实体执行一些操作
	}
}

engine.addSystem(handleEnemies)
```

## 组件 Schema

schema 描述组件内部数据的结构。一个组件可以存储任意多个字段，每个字段都必须包含在 schema 的结构中。schema 也可以包含任意多层嵌套项。

schema 中的每个字段都必须包含类型声明。你只能使用 SDK 提供的特殊 schema 类型。例如，使用类型 `Schemas.Boolean` 而不是类型 `布尔值`。输入 `Schemas.` ，你的 IDE 就会显示所有可用选项。

```ts
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```

上面的示例定义了一个组件，其 schema 保存两个值，一个 `spinning` 布尔值和一个 `speed` 浮点数。

你可以选择在定义组件时直接内联创建 schema，或者为了更易读，也可以先创建它，再进行引用。

```ts
// 方案 1：内联定义
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// 方案 2：分别定义 schema 和组件

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// 组件
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema
)
```


> [!NOTE]
> **💡 提示**：在创建组件实例时，VS Studio 的自动补全选项会在你按下 *Ctrl + Space*.


### 默认 Schema 类型

以下基本类型可在 schema 的字段中使用：

* `Schemas.Boolean`
* `Schemas.Byte`
* `Schemas.Double`
* `Schemas.Float`
* `Schemas.Int`
* `Schemas.Int64`
* `Schemas.Number`
* `Schemas.Short`
* `Schemas.String`
* `Schemas.Entity`

以下复杂类型也存在。它们都包含一系列带数值的嵌套属性。

* `Schemas.Vector3`
* `Schemas.Quaternion`
* `Schemas.Color3`
* `Schemas.Color4`


> [!NOTE]
> **💡 提示**：参见 [几何体类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/special-types/) 和 [颜色类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/) 以了解这些数据类型如何发挥作用的更多细节。


例如，你可以像这样在组件中使用这些 schema 类型来跟踪实体的渐进式移动。这个组件将初始位置和最终位置分别存储为 Vector3 值，同时将速度和已完成路径的比例存储为 float 数值。参见 [移动实体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/#move-between-two-points) 以查看此示例的完整实现。

```ts
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
```

### 数组类型

要将字段类型设为数组，请使用 `Schemas.Array()`。将数组中元素的类型作为参数传入。

```ts
const MySchema = {
	numberList: Schemas.Array(Schemas.Int),
}
```

当你使用 `MyComponent.get()`读取数组字段时，该数组是只读的。像 `.push()`这样会就地修改数组的方法不可用。请使用 `MyComponent.getMutable()` 当你需要更改其内容时。

### 可选字段

使用 `Schemas.Optional()` 以允许字段持有一个值或 `undefined`.

```ts
const MySchema = {
	playerId: Schemas.Optional(Schemas.String),
	score: Schemas.Optional(Schemas.Int),
}
```

仅 `undefined` 被视为“未设置”。像 `false`, `0`，和 `''` 会按原样存储并读取。

### 嵌套 schema 类型

要将字段类型设为对象，请使用 `Schemas.Map()`。将此对象的内容作为参数传入。这个嵌套对象本质上本身就是一个 schema，嵌套在父 schema 内。

```ts
const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: Schemas.Map({
		nestedField1: Schemas.Boolean,
		nestedField2: Schemas.Boolean,
	}),
}
```

或者，为了更易读且便于复用，你可以先单独定义嵌套 schema，然后在定义父 schema 时引用它。

```ts
const MyNestedSchema = Schemas.Map({
	nestedField1: Schemas.Boolean,
	nestedField2: Schemas.Boolean,
})

const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: MyNestedSchema,
}
```

### 枚举类型

你可以将 schema 中某个字段的类型设为枚举。枚举可以轻松在有限数量的选项之间进行选择，并为每个选项提供可读的值。

要将字段类型设为枚举，首先必须定义该枚举。然后你可以使用 `Schemas.EnumNumber` 或 `Schemas.EnumString`，具体取决于枚举的类型。这些函数接受两个参数：要引用的枚举，以及此字段要使用的默认值。

```ts
//// 字符串枚举

// 定义枚举
enum Color {
	Red = 'red',
	Green = 'green',
	Pink = 'pink',
}

// 定义一个在字段中使用此枚举的组件
const ColorComponent = engine.defineComponent('Color', {
	color: Schemas.EnumString<Color>(Color, Color.Red),
})

// 在实体上使用该组件
ColorComponent.create(engine.addEntity(), { color: Color.Green })

//// 数字枚举

// 定义枚举
enum CurveType {
	LINEAR,
	EASEIN,
	EASEOUT,
}

// 定义一个在字段中使用此枚举的组件
const CurveComponent = engine.defineComponent('curveComponent', {
	curve: Schemas.EnumNumber<CurveType>(CurveType, CurveType.LINEAR),
})

// 在实体上使用该组件
CurveComponent.create(engine.addEntity(), { curve: CurveType.EASEIN })
```

### 可互换类型

你可以将 schema 中某个字段的类型设为遵循 `oneOf` 模式，从而可以接受不同类型。

```ts
const MySchema = {
	myField: Schemas.OneOf({ type1: Schemas.Vector3, type2: Schemas.Quaternion }),
}

export const MyComponent = engine.defineComponent('MyComponent', MySchema)
```

在创建组件实例时，你需要用 `$case`来指定所选类型，例如：

```ts
MyComponent.create(myEntity, {
	myField: {
		$case: 'type1',
		value: Vector3.create(1, 1, 1),
	},
})
```

不设置该字段也是有效的。未设置的 `OneOf` 字段没有 `$case` ，并会读取回一个空对象， `{}`.

### 来自单一类型的组件

组件不一定非要持有由多个字段组成的对象。要定义一个只持有单个值的组件，请使用 `engine.defineComponentFromSchema()` 并直接传入类型：

```ts
// 每个实体持有一个数字的组件
export const Score = engine.defineComponentFromSchema('my-scene::Score', Schemas.Int)

// 每个实体持有一个数字列表的组件
export const History = engine.defineComponentFromSchema(
	'my-scene::History',
	Schemas.Array(Schemas.Int)
)
```

它们的行为与其他组件一样，包括在存储值为假值时也是如此。 `Score` 为 `0` 是一个存在且持有 `0`，而不是一个缺失的组件。

## 默认值

在组件中使用默认值通常是个好习惯，这样在每次创建新副本时就无需显式设置每个值。

该 `engine.defineComponent()` 函数接受第三个参数，允许你传入一个包含默认值的对象。这个对象可以包含 schema 中的部分或全部值。未被默认值覆盖、也未在初始化组件副本时提供的字段，将以类似 `0`, `false`这样的零值，或空字符串进行初始化，具体取决于类型。

```ts
// 定义

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// 默认值
const myDefaultValues = {
	spinning: true,
	speed: 1,
}

//// 组件
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema,
	myDefaultValues
)

// 用法
export function main() {
	//// 创建实体
	const wheel = engine.addEntity()
	const wheel2 = engine.addEntity()

	//// 使用默认值初始化组件
	WheelSpinComponent.create(wheel)

	//// 使用一个自定义值初始化组件，其余值使用默认值
	WheelSpinComponent.create(wheel2, { speed: 5 })
}
```

上面的示例创建了一个 `WheelSpinComponent` 组件，其中同时包含 schema 和一组要使用的默认值。然后，如果你在初始化该组件的副本时不指定任何值，它将使用默认值中设置的那些。

## 订阅变更

一个常见的用例是：仅在某个组件中的数据发生变化时才运行函数。使用 [OnChange](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/subscribe-to-changes/) 函数即可避免定义系统，以及显式地将旧值与新值进行比较。

```ts
export function main() {
	// 创建实体等

	WheelSpinComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.speed)
		console.log(componentData.spinning)
	})
}
```

## 构建使用组件的系统

在你的组件已定义并添加到场景中的实体后，你可以创建 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 来执行逻辑，利用组件中存储的这些数据。

```ts
// 定义组件
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// 用法
export function main() {
	// 创建实体
	const wheel1 = engine.addEntity()
	const wheel2 = engine.addEntity()

	// 创建组件实例
	WheelSpinComponent.create(wheel1, {
		spinning: true,
		speed: 10,
	})

	WheelSpinComponent.create(wheel2, {
		spinning: false,
		speed: 0,
	})
}

// 定义一个系统来遍历这些实体
export function spinSystem(dt: number) {
	// 遍历所有带有 WheelSpinComponent 的实体
	for (const [entity, wheelSpin] of engine.getEntitiesWith(
		WheelSpinComponent
	)) {
		// 仅当 spinning == true 时才执行操作
		if (wheelSpin.spinning) {
			// 获取一个可变的 Transform 组件
			const transform = Transform.getMutable(entity)

			// 相应地更新旋转值
			transform.rotation = Quaternion.multiply(
				transform.rotation,
				Quaternion.fromAngleAxis(dt * wheelSpin.speed, Vector3.Up())
			)
		}
	}
}

// 将系统添加到引擎
engine.addSystem(spinSystem)
```

上面的示例定义了一个系统，它会遍历所有包含自定义 `wheelSpinComponent`的实体，并在游戏循环的每一帧轻微旋转它们。此旋转的幅度与每个实体组件实例中存储的 `speed` 值成正比。该示例使用了 [组件查询](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/) 来仅获取相关实体。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
