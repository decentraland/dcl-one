> 在 UI 实体中写入文本。

# UI 文本

通过创建一个来向你的 UI 添加文本 `标签` 实体的父级。

A `标签` 实体具有以下可配置字段：

* `值`：要显示的字符串
* `字体大小`：文本大小，数值形式。

  > 注意： `字体大小` 不受其实体或父实体大小的影响。
* `颜色`：文本颜色，类型为 [Color4](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/).
* `字体`：要使用的字体。支持的值有：
  * `'serif'`
  * `'sans-serif'` *（默认）*
  * `'monospace'`
* `fontSrc`：指向一个 `.ttf` 场景中的字体文件，用自定义字体渲染文本。 `字体` 将作为回退字体使用。参见 [自定义字体](#custom-fonts).
* `文本对齐`：文本将如何与其父级对齐。其值取自 `TextAlignType` type. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
* `textWrap`：设置文本是否使用换行以避免超过最大宽度。默认开启（`'wrap'`），要关闭它请传入值 `'nowrap'`.


> [!WARNING]
> **📔 注**： `字体大小` 不受其实体或父实体大小的影响。


A `标签` 实体也可以拥有其他 UI 实体类型中常见的组件，例如 `uiTransform` 和 `uiBackground`.

***ui.tsx 文件：***

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 'auto', height: 'auto' }}>
		<Label
			value="这是一个标签"
			color={Color4.Red()}
			fontSize={29}
			font="sans-serif"
			textAlign="top-left"
		/>
	</UiEntity>
)
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


> [!WARNING]
> **📔 注**：本页以下所有代码片段都假设你已经有一个 `.ts` 类似于上面的，并正在运行 `ReactEcsRenderer.setUiRenderer()` 函数。


如果一行文本太长，无法适配分配的宽度或其容器的最大宽度，文本将继续显示在下一行。你可以通过更改 `textWrap` 属性设为 `'nowrap'`.

你也可以通过显式添加 `\n` 到字符串中来强制换行。

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 700, height: 400 }}>
		<Label 
			value="你好，世界！\n这一段内容相当长。它可能无法在单行中显示，所以会在某处换行。\n第四行"
			textWrap= {`nowrap`} 
		/>
	</UiEntity>
)
```

如果没有显式 `高度` 或 `宽度` 设置在 `uiTransform` 容器上，容器将使用值 `auto`，它会自动调整以适应所有文本。你可以设置一个 `maxWidth` 以及一个 `maxHeight` 以确保它不会超过某些限制。你也可以使用 `minWidth` 和 `minHeight` 以确保容器不会变得太小，即使文本较短也是如此。


> [!WARNING]
> **📔 注**：不要依赖文本的自动适配。一个 `标签` 在你不设置其大小时占用多少空间，会因浏览器而异。基于 Bevy 的浏览器会测量渲染后的文本并据此布局。Unity 浏览器几乎不给它任何高度，但仍会绘制字母，因此堆叠的标签会彼此重叠，而一个按其文本尺寸设定大小的父级会塌缩为空。
> 
> 为每个 `标签` 显式的 `宽度` **和** `高度` 在其 `uiTransform`，并为任何堆叠标签的容器显式指定高度。对于自动换行文本，请按行数设置高度：在 `fontSize: 20` 大约需要 `height: 60`.
> 
> 由于这在不同浏览器中有所不同，在一个客户端中看起来正确的预览，并不能证明在另一个客户端中布局也正确。


## 自定义字体

要渲染一个 `标签` 使用你自己的字体，请添加一个 TrueType（`.ttf`）字体文件到你的场景文件夹，并将 `fontSrc` 属性到其路径，路径相对于场景根文件夹。保留 `字体` 也要设置：当自定义字体无法加载时，它将作为回退字体使用。


> [!WARNING]
> **📔 注**: `fontSrc` 需要 `@dcl/sdk` 7.31.0 或更高版本。在更早版本中该属性不存在。


```tsx
<Label
	value="欢迎！"
	fontSize={40}
	uiTransform={{ width: 300, height: 60 }}
	fontSrc="assets/fonts/Lobster-Regular.ttf"
	font="sans-serif"
/>
```

该 `按钮` 实体， `uiText` 中提供一个补间定义列表，作为一个 `UiEntity` 以及 [`输入框` 和 `下拉菜单`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_special_types/) 实体也接受 `fontSrc` ，而且 [`TextShape`](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text/#custom-fonts) 用于 3D 空间中文本的组件。

使用自定义字体时请记住以下几点：

* **仅 `.ttf` 支持文件**。具有 CFF 描边的 OpenType 字体（通常 `.otf`）、字体集合和网页字体会被拒绝，文本将使用回退 `字体`.
* **字体必须是你场景中的一个文件**. `fontSrc` 不能指向外部 URL，字体族名称（例如来自 Google Fonts 的）也不受支持。下载该 `.ttf` 文件并将其包含在场景文件夹中。确保其许可证允许再分发。
* **在桌面客户端中，自定义字体只有在场景资源优化后才会显示**。当你发布场景时，其字体会与 3D 模型一起转换为优化后的资源包，转换完成后自定义字体才会出现。在此之前，文本会使用回退字体 `字体`。本地预览也是如此：只有在使用优化资源预览时，自定义字体才会显示（ **压缩资源** Creator Hub 中的选项，或者 `npm run start -- --asset-bundles` （通过 CLI）。参见 [使用优化后的资源预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets)。Bevy Web 客户端会直接加载该 `.ttf` 文件，因此在那里自定义字体会立即显示，在本地预览中也是如此。
* **不支持移动端**。Decentraland 移动应用会忽略 `fontSrc` 并渲染回退字体 `字体`。请选择一个看起来仍然合适的回退字体，不要依赖自定义字体来让文本适配固定空间。参见 [移动端缺失的功能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/missing-features/).
* **一个文件对应一个字重**。对于粗体和常规文本，请同时添加 `.ttf` 文件，并让每段文本指向各自的文件。
* **Creator Hub UI Designer 不会显示或编辑 `fontSrc`**。请在代码中手动将其添加到元素中。UI Designer 只会更改你在其中编辑的属性，因此你编写的 `fontSrc` 内容会保持不变。其画布不会显示自定义字体。
* **字体不包含的字符** 会使用内置的 sans-serif 字体绘制。字体中包含的非拉丁字符会在首次出现时渲染，这可能会导致短暂卡顿。

有关限制的完整列表，例如最大文件大小，请参见 [自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text/#custom-fonts) 文本页面中的

## 不要在 UI 文本中使用表情符号

不要在任何 `标签` 或 `按钮` `值`, `uiText.value`, `输入框` `占位符`，以及 `下拉菜单` 选项。

SDK 不包含表情符号字形。表情符号是否显示取决于各浏览器所捆绑的字体，而 Unity 浏览器没有这些字形，因此该字符会显示为空框或完全消失。同样的注意事项也适用于其他装饰性 Unicode，例如箭头和制表符号字符。

对于图形化标签，请改为提供图像资源：在一个小的 `UiEntity` 放在文本旁边，使用一个 `uiBackground` 带有一个 `纹理`。参见 [UI 背景](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_background/).

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			minWidth: 100,
			maxWidth: 300,
			height: 'auto',
			alignSelf: 'center',
			padding: 10,
		}}
		uiBackground={{
			color: Color4.Red(),
		}}
		uiText={{
			value: '你好，世界！',
			fontSize: 18,
		}}
	/>
)
```

## 响应式文本大小

使用 `scaleFontSize()` 函数来提供会根据玩家屏幕大小调整的字体值。在设置文本 UI 实体的 `字体大小` 属性时，请传入这个函数，而不是单个数字。

```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: '你好，世界！',
      fontSize: scaleFontSize(15)
    }}
  />
)
```

该 `scaleFontSize()` 函数需要两个参数：

* `字体大小`：要使用的基础字体大小。
* `scaleUnit` *（可选）*：缩放因子。这决定文本应基于屏幕宽度还是高度进行调整，以及调整幅度的乘数。默认： `0.39` （一个数字，因此会被解释为相对于 *宽度*）。其值可以是：
  * *数字*：一个简单数字，在这种情况下会被解释为相对于 *宽度*
  * *以 **vw***：这会使该数值相对于视口宽度。例如 `"0.8vw"`
  * *以 **vh***：这会使该数值相对于视口高度。例如 `"0.8vh"`


> [!NOTE]
> **💡 提示**：此函数的工作方式类似于 CSS `calc()` 函数。


……的值 `scaleUnit` 是窗口宽度或高度的百分比。因此 `scaleUnit` 的 `"100vw"` 是屏幕宽度的 100%，值 `"0.5vw"` 是屏幕宽度的 0.5%。

……遵循的公式 `scaleFontSize()` 是将屏幕宽度或高度乘以 `scaleUnit`，再加上 `字体大小` 第一个参数传入的值。

```ts
最终字体 = 字体大小 +（屏幕宽度 * scaleUnit / 100）
```

例如，下面的代码片段使用一个 `scaleUnit` 值为 0.8 的参数。如果屏幕宽度是 *1280px*，那么文本大小将为 **25.24**，按照以下公式计算 `15 + (1280 * 0.8 / 100)`.


> [!WARNING]
> **📔 注**: `scaleFontSize()` 会返回一个 **数字**，因此其结果是一个以 *虚拟* 像素为单位的大小——它仍然会乘以 [UI 缩放系数](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) 后再绘制。在上面的示例中，使用默认的 `1920x1080` 虚拟屏幕和一个宽 1280px 的画布时，缩放系数是 `1280 / 1920 = 0.667`，因此文本渲染大小约为 16.8px。如果你想要一个直接按画布测量、忽略虚拟屏幕的字体大小，请传入一个 `vw`/`vh` **字符串** 直接传给 `字体大小` ，例如 `fontSize: '1.8vh'`.


```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: '你好，世界！',
      fontSize: scaleFontSize(15, 0.8)
    }}
  />
)
```


> [!NOTE]
> **💡 提示**：如果你没有不同的屏幕尺寸可供测试，可以尝试使用 Web Explorer，并调整你运行预览的窗口大小。每次更改窗口时，文本都会立即调整。


作为 `scaleFontSize()`的替代方案，传入一个 `vw`/`vh` 字符串，直接作为 `字体大小` ——例如 `fontSize: '1.8vh'` ——它会根据画布对文本进行缩放，并完全忽略 UI 缩放系数。不要 **不是** 将字体大小乘以你自己根据 `UiCanvasInformation`计算出的系数； [响应式 UI 大小](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui-positioning/#responsive-ui-size).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
