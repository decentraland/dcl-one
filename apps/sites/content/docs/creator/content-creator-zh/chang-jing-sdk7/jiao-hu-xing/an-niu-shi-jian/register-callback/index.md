> 了解如何处理场景中的用户点击。

# 注册回调

处理按钮事件最简单的方法是为特定实体注册一个回调函数。每次使用特定按钮与该实体交互时，都会调用该回调函数。

如果你需要为多个相似实体添加相同的行为，可以考虑使用 [基于系统的](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 方法，而不是为每个实体都添加回调。基于系统的方法在你遍历相似实体列表时会更高效。

注册回调方式在你想描述一种只影响单个实体的行为时尤其有用，因为它更直接。


> [!WARNING]
> **📔 注意**：\
> 对于一个实体来说，要具有交互性，它 **必须** 必须有一个 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)。参见 [障碍物](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#obstacles) 。


## 指针按下

使用 `pointerEventsSystem.onPointerDown()` 来检测特定按钮的按下。

此语句需要两个参数：

* `数据`：一个包含以下内容的对象：
  * `实体`：要处理的实体
  * `opts`：一个带有可选附加数据的对象：
    * `button`：要监听哪个按钮。请参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。如果未指定按钮，则会监听所有按钮，包括前进和跳跃之类的移动按钮。
    * `maxDistance`：实体与玩家的 **化身**之间的最大距离，单位为米。默认值为 10。请参见 [距离限制](#distance-limits).
    * `maxCameraDistance`：实体与 **活动摄像机**之间的最大距离，单位为米。默认未设置。请参见 [距离限制](#distance-limits).
    * `hoverText`：要在悬停反馈提示中显示的字符串。默认情况下为“交互”。
    * `showFeedback`：如果为 false，则会隐藏此实体的悬停提示和边缘高亮。 *true* 默认情况下。
    * `showHighlight`：如果为 true，玩家将会在将光标悬停在实体上时看到边缘高亮。 *true* 默认情况下。只有在以下情况下才会考虑此值： `showFeedback` 是 *true*.
* `cb`：每次指针按下事件在指向该实体时发生时要运行的回调函数

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: '点击' },
	},
	function () {
		console.log('点击了实体')
	}
)
```

上述命令会保留已注册的回调函数，并且每次相关按钮事件发生时都会调用它。请注意，回调必须是同步函数； `async` 函数不受此处支持，并会导致错误。


> [!WARNING]
> **📔 注意**：\
> 只能有一个 `pointerEventsSystem.onPointerDown` 可以为每个实体注册。一旦添加，它就会持续监听事件，直到监听器被移除。不要在系统中反复运行此操作，因为那样会不断重写指针事件行为。


## 悬停反馈

非常重要的是，要给玩家一些提示，表明实体可以交互。

在使用 `EventsSystem`注册输入动作时，默认情况下玩家会看到：

* 实体上的边缘高亮
* 光标旁的悬停提示，其中带有他们需要按下的按钮图标，以及一段写着“交互”的字符串。

这些元素可以切换并自定义。

UI 上的悬停反馈会根据你在 `button` 字段中选择的输入显示不同的图标。在 PC 上，它会显示一个带有 `E` 用于 `InputAction.IA_PRIMARY`，一个 `F` 用于 `InputAction.IA_SECONDARY`，以及一个鼠标用于 `InputAction.IA_POINTER`.

通过修改 `hoverText` 值来更改字符串。保持此字符串简短，以便快速阅读并且不会对屏幕造成太大干扰。

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: '打开门' },
	},
	function () {
		// 打开门
	}
)
```

要隐藏悬停提示但保留边缘高亮，请将 `hoverText` 的值设为“”。

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, hoverText: ''}},
  function () {
    console.log("点击了惊喜交互物品")
  }
)
```

要隐藏边缘高亮但保留悬停提示，请设置 `showHighlight` 设置为 *false*.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: {
			button: InputAction.IA_PRIMARY,
			hoverText: '打开门',
			showHighlight: false,
		},
	},
	function () {
		console.log('打开了秘密门')
	}
)
```

要同时隐藏悬停提示和边缘高亮，请设置 `showFeedback` 设置为 *false*。这样做时，光标不会显示任何图标、文本或边缘高亮。

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, showFeedback: false}},
  function () {
    console.log("打开了秘密门")
  }
)
```

### 更改现有反馈

在使用 `EventsSystem`，这会在幕后创建一个 `PointerEvents` 组件并将其添加到交互实体上。此组件处理 UI 悬停提示的行为。要更改悬停反馈的行为，请修改此组件。请参见 [显示反馈](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/#show-feedback) 以了解如何处理此组件。

```ts
const hoverFeedback = PointerEvents.getMutable(myEntity)

if (hoverFeedback.pointerEvents[0]?.eventInfo) {
	hoverFeedback.pointerEvents[0].eventInfo.hoverText = '关门'
}
```

## 距离限制

默认情况下，玩家只有在其 **化身** 距离它在 10 米以内时才能与实体交互。如果玩家离得太远，实体的高亮效果会变成红色而不是绿色，并且不会触发指针事件。

使用 `maxDistance` 来更改该范围：

```ts
// 只有当玩家的化身距离在 5 米以内时才能点击
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 5 } },
	function () {
		console.log('点击了实体')
	}
)
```

### 按摄像机距离限制

`maxCameraDistance` 从 **当前活动摄像机** 而不是化身来测量。 [虚拟摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#using-virtual-cameras) 当你的场景使用一个放置在玩家之外的

```ts
// 只有当活动摄像机距离在 15 米以内时才能点击
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxCameraDistance: 15 } },
	function () {
		console.log('点击了实体')
	}
)
```

### 两个限制如何结合

这两个选项是独立检查。运行哪些检查取决于你设置了哪些：

| 你设置的内容                | 玩家需要满足的条件                  |
| --------------------- | -------------------------- |
| 都不设置                  | 化身距离在 10 米以内（默认值）          |
| `maxDistance` 仅       | 化身距离在 `maxDistance`        |
| `maxCameraDistance` 仅 | 摄像机距离在 `maxCameraDistance` |
| 两者都设置                 | **任一** 检查通过即可              |

同时设置两者，表示“足够近，可以走过去， **或** 足够近，可以清楚地看到它”：

```ts
// 当化身距离在 3 米以内，或者摄像机距离在 20 米以内时可点击
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 3, maxCameraDistance: 20 } },
	function () {
		console.log('点击了实体')
	}
)
```


> [!WARNING]
> **📔 注意**: `maxCameraDistance` 需要 `@dcl/sdk` 7.28.0 及更高版本。Bevy 基础的浏览器已支持此功能。如果你的场景今天必须在每个客户端上表现完全一致，请依赖 `maxDistance` 并将 `maxCameraDistance` 视为增强功能。


### maxPlayerDistance 已弃用

`maxPlayerDistance` 是 `maxDistance`的已弃用别名：两者都从化身测量距离。在新场景中请使用 `maxDistance` 。

如果场景同时设置了两者，则会使用 **较大的** 那个作为化身距离限制。

## 指针抬起

使用 `pointerEventsSystem.onPointerUp` 来注册一个回调函数，当所指示的玩家在指向实体时松开按钮，该函数会被调用。

```ts
pointerEventsSystem.onPointerUp(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: '松开按钮' },
	},
	function () {
		console.log('松开按钮')
	}
)
```

此语句需要两个参数：

* `数据`：一个包含以下内容的对象：
  * `实体`：要处理的实体
  * `opts`：一个带有可选附加数据的对象：
    * `button`：要监听哪个按钮。请参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 以了解支持的选项。如果未指定按钮，则会监听所有按钮，包括前进和跳跃之类的移动按钮。
    * `hoverText`：要在悬停反馈提示中显示的字符串。默认情况下为“交互”。
    * `showFeedback`：如果为 false，则会隐藏此实体的悬停提示。 *true* 默认情况下。
    * `maxDistance`：玩家的化身与实体之间可保持多远距离仍能与其交互，单位为米。默认值为 10。如果玩家离得太远，则不会有悬停反馈，指针事件也不会生效。请参见 [距离限制](#distance-limits).
* `cb`：每次在指向实体时发生按钮抬起事件时要运行的回调函数。

同一个实体可以注册两个不同的回调，一个用于 `pointerEventsSystem.onPointerDown` ，一个用于 `pointerEventsSystem.onPointerUp`。实体每种只能注册一个回调， [处理多个按钮](#handle-multiple-buttons) 以在同一个回调中检测不同按钮。


> [!WARNING]
> **📔 注意**：按钮抬起事件的悬停反馈只有在按钮当前正被按住时才会显示。如果玩家在未按住按钮的情况下指向实体，他们将看不到任何反馈，或者会看到按钮按下事件的反馈（如果有的话）。


## 悬停进入和离开

使用 `pointerEventsSystem.onPointerHoverEnter` 来在玩家光标开始指向某个实体时运行回调，以及 `pointerEventsSystem.onPointerHoverLeave` 在光标停止指向它时运行回调。

```ts
pointerEventsSystem.onPointerHoverEnter(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('光标开始悬停在实体上')
    }
)

pointerEventsSystem.onPointerHoverLeave(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('光标停止悬停在实体上')
    }
)
```

这些回调对于自定义悬停效果很有用，例如在玩家瞄准实体时播放声音或为实体添加动画。

## 移除回调

要移除回调函数，请使用相应的移除函数：

```ts
pointerEventsSystem.removeOnPointerDown(myEntity)
pointerEventsSystem.removeOnPointerUp(myEntity)
pointerEventsSystem.removeOnPointerHoverEnter(myEntity)
pointerEventsSystem.removeOnPointerHoverLeave(myEntity)
```

一旦移除，实体上的悬停反馈应不再显示，并且该实体应不再可交互。

每个移除函数只会移除其对应注册函数所添加的条目。重复注册和移除同一个回调不会在实体上留下过时条目。

## 近距离交互

对于基于玩家接近程度而不是光标瞄准触发的交互，请参见 [**近距离事件**](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/)。 `pointerEventsSystem` 包含等效的辅助函数（`onProximityDown`, `onProximityUp`, `onProximityEnter`，以及 `onProximityLeave`），其模式与本页所述函数相同。

同一个实体可以同时携带相同事件类型的光标处理器和近距离处理器。两个回调都会运行，各自在满足自己的条件时触发。

### 来自输入动作的数据

从输入动作中获取数据，例如被按下的按钮、被击中的实体、射线的方向和长度等。请参见（[查看文档](https://github.com/decentraland/docs/tree/main/README.md)）以了解所有可用数据的说明。

要获取这些数据，请向回调函数传递一个参数。此参数包含有关输入事件的完整数据结构。

```ts
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { button: InputAction.IA_PRIMARY } },
	function (cmd) {
		console.log(cmd.hit?.entityId)
	}
)
```

### 处理多个按钮

你不能为单个实体注册多于一个 `onPointerDown` 。理想情况下，你应该使用 [基于系统的](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 方法，因为这样你就可以处理任意多种不同输入，并为每个按钮显示一个 UI 悬停反馈提示。

作为另一种选择，你可以使用注册回调方式，并将 `button` 字段设为 `InputAction.IA_ANY`.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_ANY}},
  function (cmd) {
      if(cmd.button === InputAction.IA_POINTER){
        // 执行 X
      } else if (cmd.button === InputAction.IA_PRIMARY){
        // 执行 Y
      }
  }
)
```

这种方法并不理想，因为悬停提示只显示单个字符串，并不会指明要激活的动作。请注意，这会使回调函数对每个输入动作都运行，包括移动按键，因此你必须只过滤出你关心的动作。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
