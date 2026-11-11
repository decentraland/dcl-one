> 处理 UI 实体上的按钮事件。

# UI 按钮事件

要在你的 UI 中制作一个按钮，请创建一个 `Button` 具有以下属性的 UI 元素：

* `value`：要显示在按钮上的文本字符串。
* `onMouseDown`：每当用户按下实体上的指针按钮时都会运行的回调函数。
* `uiTransform`：UI 元素的位置属性。

下面的示例展示了如何创建一个可点击的 UI 按钮。

***ui.tsx 文件：***

```tsx
import { Button } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on the UI')
		}}
	/>
)
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 注意**：本页中的所有以下代码片段都假设你已经有一个 `.ts` 类似于上面的文件，正在运行 `ReactEcsRenderer.setUiRenderer()` 函数。


你也可以将点击时执行的函数写在 UI 定义之外，并通过名称引用它。这有助于让 UI 代码更易读，而且当多个可点击的 UI 实体需要调用同一个函数时也很有用。

```tsx
import { Button } from '@dcl/sdk/react-ecs'

function handleClick() {
	// 点击时执行一些操作
	console.log('Clicked on the UI')
}
export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100 }}
		onMouseDown={handleClick}
	/>
)
```

以下字段可以添加到一个 `Button` UI 元素：

* `onMouseDown`：每当用户按下实体上的指针按钮时都会运行的回调函数。
* `onMouseUp`：每当指针悬停在实体上方时松开按钮都会运行的回调函数。
* `onMouseEnter`：每当指针开始悬停在按钮上方时都会运行的回调函数。
* `onMouseLeave`：每当指针停止悬停在按钮上方时都会运行的回调函数。
* `color`：按钮上文本的颜色。
* `font`：按钮上文本的字体。
* `textAlign`：按钮内文本的对齐方式
* `uiTransform`：UI 元素的位置属性。
* `uiBackground`：设置 UI 元素的颜色或纹理。
* `variant`：使用此属性将按钮样式设置为默认样式之一。 `primary` 和 `secondary` 可用。
* `disabled`：用于将按钮设为禁用的布尔值。当 `disabled` 设置为 *true*时， `onMouseDown` 和 `onMouseUp` 动作将不再被调用，按钮也不再响应任何指针交互。按钮还会以“灰化”方式绘制：文本和背景的 `alpha` 值都只会以一半显示。这个变化仅影响显示效果。场景传入的 `Color4` 值不会被修改，因此你可以安全地在多个元素之间复用共享调色板对象。

## 按钮样式

将 variant 设为 `primary` 或 `secondary` ，即可利用按钮的默认样式选项。 `primary` 会让你的按钮变成白色文字的红色按钮， `secondary` 会让你的按钮变成红色文字的白色按钮。

```tsx
import { UiEntity, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: 500,
			height: 230,
			margin: '16px 0 8px 270px',
			padding: 4,
			alignSelf: 'center',
		}}
		uiBackground={{ color: Color4.Gray() }}
	>
		<Button
			value="Click Me"
			variant="primary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
		<Button
			value="Click Me"
			variant="secondary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
	</UiEntity>
)
```

你也可以自由使用背景上的所有属性。你还可以先设置一个 variant，然后覆盖其中的一些属性。此示例使用 `primary` variant，但将颜色覆盖为绿色：

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<Button
		value="My Button!"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
		}}
		uiBackground={{
			color: Color4.Green(),
		}}
	/>
)
```

## 可切换按钮

一个常见用例是让按钮在两种状态之间切换，就像开关一样。下面的示例每次按下按钮时都会在两种颜色之间切换：

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
			buttonEnabled = !buttonEnabled
			if (buttonEnabled) {
				// 执行一些操作
			} else {
				// 执行其他操作
			}
		}}
		uiBackground={{
			color: buttonEnabled ? Color4.Green() : Color4.Red(),
		}}
	/>
)
```

请注意，在上面的示例中，颜色取决于一个 `buttonEnabled` 变量。只要这个变量的值发生变化，它就会立即影响背景颜色。

## 悬停反馈

另一个常见用例是在悬停按钮时显示某种视觉提示，以说明它可以交互，甚至显示一个解释该按钮作用的悬停提示。使用 `onMouseEnter` 和 `onMouseLeave` 回调来检测玩家的光标是否在按钮上，并做出相应反应。

```tsx
import { Button } from '@dcl/sdk/react-ecs'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			// 按钮函数
		}}
		onMouseEnter={() => {
			// 显示提示
		}}
		onMouseLeave={() => {
			// 隐藏提示
		}}
	/>
)
```

## 让其他元素可点击

UI 中的任何元素都可以通过添加一个 `onMouseDown` 属性使其可点击，其工作方式与按钮完全相同。下面的示例为背景图片和文本添加了 `onMouseDown` 属性。

```tsx
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
	<UiEntity
		onMouseDown={() => {
			console.log('Background clicked!')
		}}
		uiTransform={{
			width: 400,
			height: 230,
		}}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	>
		<Label
			onMouseDown={() => {
				console.log('Label clicked!')
			}}
			value={`Player: ${getPlayerPosition()}` }
			fontSize={18}
			uiTransform={{ width: '100%', height: 30 }}
		/>
	</UiEntity>
)

function getPlayerPosition() {
	const playerPosition = Transform.getOrNull(engine.PlayerEntity)
	if (!playerPosition) return 'unknown'
	const { x, y, z } = playerPosition.position
	return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

## 指针阻挡

默认情况下，所有 UI 实体都不会阻挡指针，这意味着玩家的点击会穿过它们，并与其后方 3D 世界空间中的对象交互。如果某个实体具有一个 `onMouseDown` 回调，那么它就会变成指针阻挡，因此玩家的点击不会影响该 UI 实体后面的内容。

你可以通过更改 `pointerFilter` 属性的值来改变这一默认行为， `uiTransform` 组件上的任意 UI 实体都可以这样做。例如，将一个没有 `onMouseDown` 的实体设置为指针阻挡。

支持的 `pointerFilter` 值有：

* `block`：该 UI 元素会阻挡指针，玩家无法点击该 UI 元素后面的任何内容。
* `none`：该 UI 元素不会阻挡指针。该元素不可点击，其后面的任何内容都可以被点击。

下面是一个没有 `onMouseDown`的简单 UI，但它通过将 `pointerFilter` 设置为 `block`.

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// 绘制 UI
export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '100%',
			height: '100px',
			pointerFilter: `block`,
		}}
		uiText={{ value: `This element is pointer blocking`, fontSize: 40 }}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	/>
)
```

### 阻挡范围遵循布局框，而不是可见像素

阻挡元素会捕获其 **整个矩形区域**上的点击，无论那里是否绘制了任何内容。完全透明的背景不会有任何影响。


> [!CAUTION]
> **警告：** 绝不要在一个全屏包装器上放置指针处理器或 `pointerFilter: 'block'` ，该包装器的尺寸是 `100%` 按 `100%`.
> 
> 它的矩形就是整个屏幕，因此布局根节点上一个不小心留下的 `onMouseDown` 就会让其他所有 UI 元素以及 3D 世界中的所有内容都无法点击。UI 看起来仍然完全正确，因为你能看到的面板只覆盖了屏幕的一小部分，这使得这个问题非常难以发现。


把处理器附加到所需的最小元素上：面板、按钮或行。布局包装器保持不带处理器。

有两种情况下，全屏阻挡元素是正确的做法，而且这两种情况都是有意为之：

* 一个 **模态背景**，其目的是吞掉点击，而且只在模态窗口打开时渲染。
* 一个 **拖拽释放捕获器**，它只会在拖拽进行时存在。参见下面的 [拖拽交互](#drag-interactions) 。

如果你发现场景中任何地方的点击都不工作了，先检查这个。

## 拖拽交互

UI 指针处理器（`onMouseDown`, `onMouseUp`, `onMouseEnter`, `onMouseLeave`）不接受参数。它们作为简单的 `() => void` 回调触发，不包含位置或坐标数据。UI 系统中没有 `onMouseDrag` 或 `onMouseMove` 处理器。

要构建基于拖拽的 UI（滑块、拖动条、拖拽手柄），请使用 `PrimaryPointerInfo.screenDelta` 来自 `@dcl/sdk/ecs`。这会给出自上一帧以来鼠标移动的像素数，并且无论光标位于何处都会每帧更新。

该模式如下运作：

1. `onMouseDown` 在拖拽目标上开始拖拽并记录初始值。
2. 一个系统读取 `screenDelta` 并在拖拽激活时把它累加到该值中。
3. 一个带有 `pointerFilter: 'block'` 的全屏不可见覆盖层捕获鼠标释放，因此即使在狭窄目标外松手也能结束拖拽。

```ts
import { engine, PrimaryPointerInfo, UiCanvasInformation } from '@dcl/sdk/ecs'

let dragging = false
let sliderValue = 0.5

// 每帧调用此系统以累积拖拽移动
engine.addSystem(() => {
	if (!dragging) return

	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta || delta.x === 0) return

	// 将屏幕像素转换为 0–1 范围
	// 除以 UI 缩放因子，使拖拽速度与光标一致
	const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
	const scale = canvas ? Math.min(canvas.width / 1920, canvas.height / 1080) : 1
	const trackWidth = 200 // 滑块轨道的虚拟像素宽度

	sliderValue = Math.max(0, Math.min(1, sliderValue + delta.x / scale / trackWidth))
})
```


> [!WARNING]
> **注意：** 在移动端上， `screenDelta` 始终返回 0（没有可自由移动的光标）。对于兼容移动端的滑块，请在拖拽轨道旁添加步进按钮（`-` / `+`）。使用 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) 来自 `@dcl/sdk/platform` 来分支你的 UI。



> [!NOTE]
> **提示：** 务必将 `screenDelta` 除以 UI 缩放因子，否则当屏幕分辨率与虚拟尺寸不同时时，拖拽会过冲或不足。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_button_events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
