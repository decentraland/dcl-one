> 了解如何处理场景中的用户点击。

# 基于系统的事件

如果你的场景中有多个相似实体，而且它们都用相同逻辑被激活，那么你可以编写一个系统来遍历它们，并且只描述一次这种行为。这也是性能最高且更 [数据导向的](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/) 方法。

你也可以使用系统来检测 [全局输入事件](#global-input-events)，这样无论玩家的光标指向哪里，只要按下某个按键，场景都会作出反应。

如果你只是想点击或按下单个实体上的按钮来激活它，最简单的方法是使用 [注册回调](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/) 方法。

如果要设置更具体的自定义逻辑，你可能需要处理原始数据并使用 [高级](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/advanced-button-events/) 方法。

一个实体要可交互，必须具有一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)显示。请参阅 [障碍物](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#obstacles) 以了解更多详情。这也适用于近距离点击事件。

## 使用系统

通过在输入上运行其中一个辅助函数来检查按钮事件 `inputSystem` 命名空间中的每个 tick 内，位于一个 [系统中](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/).

例如，以下系统使用 `inputSystem.isTriggered()` 函数来检查指针是否被点击。每个 tick 都会检查按钮是否被按下。如果 `inputSystem.isTriggered()` 返回 *true*，系统就会运行一些自定义逻辑作为响应。

```ts
engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
		// 按钮按下时的响应逻辑
	}
})
```

以下辅助函数可在 `inputSystem` 命名空间中使用，并且也可以在每个 tick 中类似地调用：

* `inputSystem.isTriggered`：如果自上一个 tick 以来发生了某个输入动作，则返回 true。
* `inputSystem.isPressed`：如果某个输入当前正被按下，则返回 true。在按钮再次松开之前，它会在每个 tick 都返回 true。
* `inputSystem.getInputCommand`：返回一个包含输入动作数据的对象。

有关各项的更多细节，请参见下面的各个部分。

在处理实体上的按钮事件时，始终要向玩家提供反馈，让玩家知道该实体可以交互。如果你添加一个 `PointerEvents` 组件到实体，玩家在将光标悬停在该实体上时会看到一个提示。参见 [显示反馈](#show-feedback) 了解如何在可交互实体上添加悬停提示。

### 全局输入事件

使用 `inputSystem.isTriggered` 来检测 SDK 跟踪的任意输入上的按下和松开事件。

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
	) {
		// 按钮按下时的响应逻辑
	}
})
```

上面的示例会检查按钮是否被按下，而不考虑指针位于何处，或路径上可能有哪些实体。

`inputSystem.isTriggered()` 返回 *true* 只有在游戏循环当前 tick 中指示的按钮被按下时才会如此。如果按钮没有被按下，或者它在上一个 tick 时就已经处于按下状态，则返回 *false*.


> [!WARNING]
> **📔 注意**：要检测到指针事件，玩家需要站在场景边界内。玩家的光标也需要被锁定；在自由光标状态下按下的按钮不会被检测到。
> 
> 你想要交互的每个实体都必须同时具有一个 [`PointerEvents` 组件](#show-feedback) 和一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).


该 `inputSystem.isTriggered` 函数接受以下必填参数：

* `InputAction`：要监听哪种输入，取值来自 `InputAction` 枚举。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。
* `PointerEventType`：要监听哪种事件类型，取值来自 `PointerEventType` 枚举。参见 [指针事件类型](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#types-of-pointer-events) 以了解支持的选项。

### 激活实体

要在指向特定实体时检测按钮事件，请向 `inputSystem.isTriggered` 传入第三个可选参数，以指定要检查的实体。

```ts
// 给实体添加一个 PointerEvents 组件
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				showFeedback: false,
			},
		},
	],
})

// 创建系统
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// 对 myEntity 上的按钮按下作出响应的逻辑
	}
})
```

上面的示例会在每个 tick 检查一个硬编码实体是否被指针按钮（鼠标左键）按下。

该 `inputSystem.isTriggered` 函数接受以下参数：

* `InputAction`：要监听哪种输入，取值来自 `InputAction` 枚举。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。
* `PointerEventType`：要监听哪种事件类型，取值来自 `PointerEventType` 枚举。参见 [指针事件类型](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#types-of-pointer-events) 以了解支持的选项。
* `Entity` *（可选）*：要在其上检查这些事件的实体。如果未提供值，则会检查该按钮的全局按下情况，而不考虑玩家光标指向哪里。

请注意，在此示例中我们还添加了一个 `PointerEvents` 组件到我们想要交互的实体上。这个步骤是必需的，没有该组件，任何 `inputSystem`的函数都无法检测到该实体。参见 [显示反馈](#show-feedback) 以了解更多关于 `PointerEvents` 组件的实体时，组件不会被激活。


> [!WARNING]
> **📔 注意**：以及一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)显示。请参阅 [障碍物](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#obstacles) 以了解更多详情。


如果有多个实体可以以相同方式与玩家交互，可以考虑使用 `inputSystem.getInputCommand`。此命令会返回有关全局点击命令的信息，包括实体 ID；你可以利用它来执行一个能够统一处理它们的函数。

```ts
engine.addSystem(() => {
	const result = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN
	)
	if (result) {
		if (result.hit?.entityId === myEntity) {
			// 处理点击
		}
	}
})
```


> [!WARNING]
> **📔 注意**：你想要交互的每个实体都必须同时具有一个 [`PointerEvents` 组件](#show-feedback) 和一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).


请参阅 [来自输入动作的数据](#data-from-input-action) 以了解更多信息。此方法还会提供有关指针事件命中的更详细数据。

### 输入按钮松开

你也可以用几乎相同的方式检查指针松开事件，使用 `PointerEventType.PET_UP`.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_UP,
			myEntity
		)
	) {
		// 在指向 myEntity 时，对按钮松开作出响应的逻辑
	}
})
```


> [!WARNING]
> **📔 注意**：在针对特定实体检查指针松开事件时，不会考虑按钮按下时光标指向哪里；它只考虑按钮松开时光标指向哪里。
> 
> 另外请记住，该实体也必须同时具有一个 [`PointerEvents` 组件](#show-feedback) 和一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).


### 检查按下的按钮

使用以下方法检查某个按钮当前是否正被按下： `inputSystem.isPressed()` 在一个系统中。

```ts
engine.addSystem(() => {
	if (inputSystem.isPressed(InputAction.IA_POINTER)) {
		// 对按钮持续按下作出响应的逻辑
	}
})
```

`inputSystem.isPressed()` 返回 *true* 如果按钮当前正被按住，则返回 true；无论按钮何时被按下，也无论玩家的光标指向哪里。否则返回 *false*.

该 `inputSystem.isPressed` 函数只接受一个参数：

* `InputAction`：要监听哪种输入，取值来自 `InputAction` 枚举。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。

### 处理多个实体

如果你的场景中有多个实体会以相同方式受指针事件影响，那么编写一个遍历它们全部的系统是合理的。

```ts
engine.addSystem(() => {
	const activatedEntites = engine.getEntitiesWith(PointerEvents)
	for (const [entity] of activatedEntites) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				实体
			)
		) {
			// 对与实体交互作出响应的逻辑
		}
	}
})
```

这个示例使用了一个 [组件查询](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/) 来遍历所有带有 `PointerEvents` 组件的实体。然后它会用 `inputSystem.isTriggered`逐个遍历它们。如果在其中任何一个实体上检测到输入动作，它就会执行自定义逻辑。

与其遍历 *全部* 带有一个 `PointerEvents` 组件的实体并放在单个系统中处理，不如编写不同的系统来处理应该以不同方式表现的实体。推荐的方法是用特定的 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/)来标记不同类型的实体，并在独立的系统中遍历它们。

```ts
engine.addSystem(() => {
	const activatedDoors = engine.getEntitiesWith(IsDoor, PointerEvents)
	for (const [entity] of activatedDoors) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				实体
			)
		) {
			openDoor(entity)
		}
	}
})

engine.addSystem(() => {
	const pickedUpGems = engine.getEntitiesWith(IsGem, PointerEvents)
	for (const [entity] of pickedUpGems) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				实体
			)
		) {
			pickUp(entity)
		}
	}
})
```

这个示例包含一个系统，它遍历所有带有名为 `IsDoor` 的自定义组件的实体，另一个系统则遍历所有带有名为 `isGem`的自定义组件的实体。在这两个系统中，它都会检查每个匹配实体，看它们是否被指针按钮激活。

这样组织场景代码非常 [数据导向的](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/) ，并且应该会非常高效地利用内存资源。

\## 实体上的多个按钮

使用像 `pointerEventsSystem.onPointerDown`，每个实体都只能拥有一种事件类型。你不能注册两个不同的按钮事件。而使用基于系统的事件时则没有这个限制。例如，下面这个实体允许玩家通过按下 E 和 F 来交互，并分别触发不同功能。

```ts
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            // 响应 E
        }
        if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN, myEntity)) {
            // 响应 F
        }
    })
```

## 显示反馈

在指向实体时显示 UI 提示，请使用实体的 `PointerEvents` 组件的实体时，组件不会被激活。

```ts
// 创建实体
const myEntity = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
			},
		},
	],
})
```

每当玩家的光标指向该实体中的碰撞体时，UI 都会显示一个悬停提示，表明该实体可以交互。有关可配置内容的详情，请参见下面各节。


> [!WARNING]
> **📔 注意**： `PointerEvents` 组件只负责显示悬停反馈。要使用自定义逻辑处理按钮事件本身，请参见 [使用系统](#using-a-system).


该 `PointerEvents` 组件至少需要一个指针事件定义。每个指针事件定义都可以按以下方式配置：

* `eventType`：要监听哪种事件类型，取值来自 `PointerEventType` 枚举。参见 [指针事件类型](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#types-of-pointer-events) 以了解支持的选项。
* `eventInfo`：一个可包含以下字段的对象：
  * `button` (*必需的*）：要监听哪种输入，取值来自 `InputAction` 枚举。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。
  * `hoverText` *（可选）*：在悬停反馈提示中显示什么字符串。默认是“Interact”。
  * `showFeedback` *（可选）*：如果为 false，则会隐藏该实体的悬停提示和边缘高亮。 *true* ，默认情况下。
  * `showHighlight` *（可选）*：如果为 true，玩家将会在光标悬停在实体上时看到边缘高亮。 *true* 默认如此。只有在以下情况下才会考虑此值： `showFeedback` 为 *true*.
  * `maxDistance` *（可选）*：玩家的 **化身** 距离实体多远时仍可与其交互，以米为单位。默认值为 *10 米*显示。请参阅 [最大距离](#max-distance).
  * `maxCameraDistance` *（可选）*： **活动摄像机** 距离实体多远时仍可与其交互，以米为单位。默认未设置。参见 [最大距离](#max-distance).
  * `priority` *（可选）*：当多个实体重叠时，哪个实体优先生效。数值越大优先级越高。 *0* ，默认情况下。

一个单独的 `PointerEvents` 组件可以包含多个指针事件定义，这些定义可以检测不同按钮的不同事件。每个实体只能有 *一个* `PointerEvents` 组件，但该组件可以在其 `pointerEvents` 数组中包含多个对象，每个对象对应一个要响应的事件。

```ts
PointerEvents.create(NPCEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Greet',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Choke',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Stop Choke',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_SECONDARY,
				hoverText: 'Give gem',
			},
		},
	],
})
```

玩家会看到多个标签，每个指针事件对应一个，围绕光标呈放射状显示。

下面的示例结合了使用 `PointerEvents` 来显示悬停提示，以及一个真正使用自定义逻辑处理玩家操作的系统。

```ts
// 创建实体
const myEntity = engine.addEntity()

// 给实体提供悬停反馈
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})

// 处理实体上的点击事件
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// 对输入动作作出响应的自定义逻辑
	}
})
```

### 悬停反馈

当玩家将光标悬停在带有一个 `PointerEvents` 组件的物品上时，他们会看到：

* 实体上的边缘高亮
* 光标附近的悬停提示，其中带有一个表示需要按下按钮的图标，以及显示“Interact”的字符串。

这些元素可以切换并自定义。

UI 上的悬停反馈会根据你在 `button` 字段中选择的输入显示不同的图标。在 PC 上，它会显示一个带有 `E` 表示 `InputAction.IA_PRIMARY`的图标， `F` 表示 `InputAction.IA_SECONDARY`的图标，以及一个鼠标图标表示 `InputAction.IA_POINTER`.

通过更改 `hoverText` 值来更改该字符串。请保持这个字符串简短，这样便于快速阅读，也不会过于干扰屏幕。

```ts
// 创建实体
const chest = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})
```

在上面的示例中，指针事件包含一个 `hoverText`的值。此字段定义了当玩家指向该实体时要在 UI 中显示的字符串。默认情况下，这个字符串显示为 *Interact*.


> [!NOTE]
> **💡 提示**： `hoverText` 字符串应描述交互时发生的动作。例如 `Open`, `Activate`, `Grab`, `选择`。这些字符串应尽可能简短，以免过多吸引玩家的注意力。


如果一个实体上有多个指针事件，那么每个事件的悬停提示都会围绕光标呈放射状显示。

该 `hoverText` 一个 `.UP` 指针事件只会在玩家已经按住对应按键并指向该实体时显示。

如果一个实体同时具有一个 `DOWN` 指针事件以及一个 `UP` 指针事件，那么 `DOWN` 操作的提示会在按钮未被按下时显示。提示会切换为来自 `UP` 事件的提示，只有在按钮被按下并保持按下时才会如此。

```ts
// 创建实体
const entity = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(entity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drag',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drop',
			},
		},
	],
})
```

要隐藏悬停提示但保留边缘高亮，请将 `hoverText` 的值设为“”。

```ts
// 创建实体
const chest = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '',
			},
		},
	],
})
```

要隐藏边缘高亮但保留悬停提示，请设置 `showHighlight` 设置为 *false*.

```ts
// 创建实体
const chest = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open door',
				showHighlight: false,
			},
		},
	],
})
```

要同时隐藏悬停提示和边缘高亮，请设置 `showFeedback` 设置为 *false*。这样做时，光标不会显示任何图标、文本或边缘高亮。你也可以直接移除该 `PointerEvents` 组件。

```ts
// 创建实体
const chest = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				showFeedback: false,
			},
		},
	],
})
```

### 最大距离

有些实体可以有意设计为只能在近距离交互。如果玩家离实体太远，悬停提示就不会显示在光标旁边。

默认情况下，只有当玩家的 **化身** 位于 *10 米* 在实体范围内时，实体才可点击。你可以通过设置 `maxDistance` 属性来更改该范围。

```ts
// 创建实体
const myEntity = engine.addEntity()

// 给实体添加一个 PointerEvents 组件
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 6,
			},
		},
	],
})

// 实现一个系统来处理该指针事件
engine.addSystem(() => {
	// 获取该指针事件的数据
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// 检查点击是否足够靠近
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// 执行某些操作
	}
})
```

上面的示例将悬停提示的最大距离设置为 *6 米*。请确保处理输入动作的逻辑也遵循相同规则。参见 [来自输入动作的数据](#data-from-input-action) 了解如何获取输入动作的距离。

`maxDistance` 是从玩家的 **化身**测量的，因此在第一人称和第三人称下表现相同。

若要根据 **活动摄像机** 的距离来限制交互，请改用 `maxCameraDistance`。同时设置两者意味着任一检查通过即可。参见 [距离限制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/#distance-limits) 以了解完整规则。


> [!WARNING]
> **📔 注意**: `maxPlayerDistance` 是已弃用的别名，对应 `maxDistance`。两者都以化身为参照测量距离，因此在新场景中请使用 `maxDistance` 。


## 高级自定义提示

该 `PointerEvents` 当玩家的光标开始悬停在实体上时，该组件可以轻松添加 UI 提示。通常，提示的表现与玩家在其他 Decentraland 场景中习惯看到的方式保持一致是好事。不过，在某些情况下，你可能希望以自定义方式表明某物可交互。例如，当玩家开始悬停在实体上时，你可以播放一个微妙的声音。你也可以在悬停期间在实体周围显示发光高亮，并在不再悬停时将其隐藏。它还可以用于特定的玩法机制。

使用消息总线的 `inputSystem.isTriggered()` 函数与 `PointerEventType.PET_HOVER_ENTER` 和 `PointerEventType.PET_HOVER_LEAVE` 事件一起使用，以便在玩家的光标开始指向实体的碰撞体时，以及光标停止指向它时执行自定义行为。

下面的示例会将实体放大到 *1.5* 当光标开始指向它们的碰撞体时，并在光标离开时将其恢复为 *1* 。

```ts
engine.addSystem(() => {
	const meshEntities = engine.getEntitiesWith(MeshRenderer)
	for (const [entity] of meshEntities) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_ENTER,
				实体
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1.5, 1.5, 1.5)
		}

		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_LEAVE,
				实体
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1, 1, 1)
		}
	}
})
```


> [!WARNING]
> **📔 注意**：你想要交互的每个实体都必须同时具有一个 [`PointerEvents` 组件](#show-feedback) 和一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).


## 近距离事件

使用以下方法检查近距离按钮按下，而不考虑他们的光标指向哪里： `PET_DOWN` 用于使用 `InteractionType.PROXIMITY`的实体上。设置 `interactionType` 字段到指针事件定义中，放在 `eventInfo`，以将该事件标记为基于接近距离。

请参阅 [**接近事件**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/) 以了解更多细节，包括可用的辅助函数。

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: '按 E 键',
				maxDistance: 5
			},
			interactionType: InteractionType.PROXIMITY,
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
		console.log('玩家在实体附近按下了按钮')
	}
})
```

使用诸如 `pointerEventsSystem.onProximityDown`，一个实体可以同时持有一个光标处理器和一个接近处理器，并且两个回调都会运行。它不能做的是为同一个处理器注册两个不同的按钮。基于系统的事件没有这种限制：你为每种行为定义一条条目。例如，这里有一个实体，可以通过将光标指向它并按 E 键进行交互，也可以在走近它并按 E 键时交互，而无需用光标指向它。

```ts
	PointerEvents.create(myEntity, {
        pointerEvents: [
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_POINTER,
                    hoverText: '点击',
                    maxDistance: 15,
                },
                interactionType: InteractionType.CURSOR,
            },
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_PRIMARY,
                    hoverText: '按 E 键',
                    maxDistance: 15,
                },
                interactionType: InteractionType.PROXIMITY,
            },
        ],
    })
    
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN, myEntity)) {
            console.log('玩家点击了实体')
        }
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            console.log('玩家在实体附近按下了按钮')
        }
    })
```

你还可以使用 `inputSystem.isTriggered()` 与 `PET_PROXIMITY_ENTER` 和 `PET_PROXIMITY_LEAVE` 事件类型来响应玩家进入或离开实体的接近范围。这对于显示接近触发实体的额外反馈很有用，例如播放声音或动画。

要使其生效，实体必须具有一个 `PointerEvents` 组件，并定义相应的接近事件类型。

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_PROXIMITY_ENTER,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
			},
		},
		{
			eventType: PointerEventType.PET_PROXIMITY_LEAVE,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
				showFeedback: false,
			},
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_ENTER, myEntity)) {
		console.log('玩家进入了接近范围')
	}
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_LEAVE, myEntity)) {
		console.log('玩家离开了接近范围')
	}
})
```

## 来自输入动作的数据

从输入动作中获取数据，例如按下的按钮、命中的实体、射线的方向和长度等。参见（[查看文档](https://github.com/decentraland/docs/tree/main/README.md)) 以了解所有可用数据的说明。

要获取这些数据，请使用 `inputSystem.getInputCommand`。此函数会返回包含输入事件数据的完整数据结构。

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```

如果没有与查询匹配的输入动作，则 `inputSystem.getInputCommand` 返回 *null*。请确保在逻辑中处理这种情况。

### 最大点击距离

为了强制限制最大距离，使实体只能在近距离被点击，请获取 `hit.length` 这一事件数据的属性。

```ts
// 实现一个系统来处理该指针事件
engine.addSystem(() => {
	// 获取该指针事件的数据
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// 检查点击是否足够靠近
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// 执行某些操作
	}
})
```


> [!WARNING]
> **📔 注意**：如果你忽略任何远距离事件，请确保设置 `maxDistance` 在 `PointerEvents` 组件上的参数，以保持行为一致。


### 模型中的不同网格

通常， *.glTF* 3D 模型由多个网格组成，每个网格都有各自独立的内部名称。所有按钮事件都包含被点击的具体网格信息，因此你可以利用这些信息在不同情况下触发不同的点击行为。

要查看模型内部各个网格的命名方式，你必须使用编辑工具打开该 3D 模型，例如 [Blender](https://www.blender.org/) 例如。

![](/docs/_remote/5f167d2485c1df5d331a16b6.webp)


> [!NOTE]
> **💡 提示**：你也可以通过将其记录到控制台并读取输出，来得知被点击的网格名称。


你可以访问 `meshName` 作为 `hit` 对象的一部分，它由点击事件返回。

在下面的示例中，我们有一个房屋模型，其中包含一个名为 `firePlace`的网格。我们只想在点击其对应网格时点亮壁炉。

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd && cmd.hit?.meshName === 'firePlace') {
		// 点燃火焰
	}
})
```


> [!WARNING]
> **📔 注意**：这仅适用于基于指针的事件，不适用于基于接近距离的事件，因为在后一种情况下，模型的所有网格都可能同时处于玩家附近。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
