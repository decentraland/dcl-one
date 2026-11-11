> 如何向场景中添加文本

# 文本

使用以下组件向场景添加文本 `TextShape` 组件。该文本位于一个位置

Decentraland 中的文本支持所有 *utf8* 字符，包括东亚和特殊字符。


> [!WARNING]
> **📔 注**: 该组件适用于场景 3D 空间中的世界内标签和 UI，而不是玩家的 2D HUD UI。


要将文本作为标签添加到现有实体上，最佳实践是创建第二个实体，使其具有 `TextShape` 组件，并将其设为另一个实体的子级。

## 在 Creator Hub 中使用场景编辑器

在场景中放置文本最简单的方法是添加一个 **Text** [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 在场景编辑器中可视化地进行。然后你可以在场景编辑器的 UI 中设置所有可用字段。

![](/docs/_remote/339b81b8af14deb7d4666a01.webp)

## 创建文本组件

以下示例展示如何创建一个 `TextShape` 组件并通过代码将其添加到实体中。

```ts
const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Hello World',
})
```


> [!WARNING]
> **📔 注**：如果带有文本组件的实体是另一个实体的子级，那么它会受到父级缩放的影响。如果父级沿其轴向缩放不均匀，文本也会被拉伸或压缩。



> [!WARNING]
> **📔 注**: `TextShape` 组件不可点击。 `PointerEvents` 当用于具有 `TextShape` 组件，使其在场景运行时可用。



> [!WARNING]
> **📔 注**: `TextShape` 必须通过以下方式导入：
> 
> > `import { TextShape } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) ，了解如何轻松处理这些内容。


## 更改文本值

创建新的文本组件时，你会为其分配一个要显示的字符串。该字符串存储在 `text` 字段中。

如果你想更改组件显示的字符串，可以随时通过修改 `text` 字段 [可变版本](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/mutable-data/) 中的组件来实现。

```ts
const mutableText = TextShape.getMutable(myEntity)

mutableText.text = 'new string'
```

## 基本文本属性

该 `TextShape` 组件有多个可用于设置文本样式的属性。以下是一些最常见的属性：

* `字体`：来自枚举的值 `Font`.
* `fontSrc`: *字符串*。指向一个 `.ttf` 字体文件的路径，用于在你的场景中以自定义字体渲染文本。参见 [自定义字体](#custom-fonts).
* `字体大小`: *数字*。字体大小为 10 的实体高 1 米。
* `文本颜色`: *Color4* 对象。 *Color4* 对象会存储一个 *RGB* 颜色，由 0 到 1 的三个数字表示，再加上 *alpha* 用于透明度。参见 [颜色类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/) 了解更多详情。

```ts
TextShape.create(sign, {
	text: 'Hello World',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```

## 字体

文本形状可以使用来自枚举 `Font`中的字体。该枚举目前包含以下字体：

* `Font.F_SANS_SERIF`
* `Font.F_SERIF`
* `Font.F_MONOSPACE`

默认使用 `Font.F_SANS_SERIF`.

```ts
TextShape.create(sign, {
	text: 'Hello World',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```


> [!NOTE]
> **💡 提示**：如果使用 VS studio 或其他 IDE，请输入 `Font.` 你应该会看到包含所有可用字体的建议列表。


### 自定义字体

要使用自己的字体渲染文本，请添加一个 TrueType（`.ttf`）字体文件到你的场景文件夹，并将 `fontSrc` 属性设置为其相对于场景根文件夹的路径（就像 `src` 一个 `GltfContainer`的 `字体` 也要设置：当自定义字体无法加载时，它将作为回退字体使用。


> [!WARNING]
> **📔 注**: `fontSrc` 需要 `@dcl/sdk` 7.31.0 或更高版本。在更早版本中该属性不存在。


```ts
TextShape.create(sign, {
	text: 'Hello World',
	fontSize: 5,
	fontSrc: 'assets/fonts/Lobster-Regular.ttf',
	font: Font.F_SANS_SERIF,
})
```

相同的 `fontSrc` 该属性存在于 UI 中 `标签`, `按钮`, `输入框` 和 `下拉菜单` 实体，以及在 `uiText` 一个 `UiEntity`。参见 [UI 中的自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#custom-fonts).

使用自定义字体时请记住以下几点：

* **仅 `.ttf` 支持文件**。该文件必须是 TrueType 字体。带有 CFF 轮廓的 OpenType 字体（通常 `.otf`）、字体集合（`.ttc`）以及网页字体（`.woff`, `.woff2`）会被拒绝，文本将使用回退字体渲染 `字体`.
* **字体必须是你场景中的一个文件**. `fontSrc` 不能指向外部 URL，字体族名称（例如来自 Google Fonts 的）也不受支持。下载该 `.ttf` 文件，将其包含在你的场景文件夹中并引用其路径。请确保该字体的许可证允许再分发；Google Fonts 上的大多数字体都使用 SIL Open Font License，而它允许再分发。
* **在桌面客户端中，自定义字体只有在场景资源优化后才会显示**。当你发布场景时，其字体会与 3D 模型一起转换为优化后的资源包，转换完成后自定义字体才会出现。在此之前，文本会使用回退字体 `字体`。在支持自定义字体之前发布的场景，只有在再次发布时才会转换其字体。局部预览也是如此：只有在使用优化后的资源预览时，自定义字体才会显示（ **压缩资源** Creator Hub 中的选项，或者 `npm run start -- --asset-bundles` （通过 CLI）。参见 [使用优化后的资源预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets)。Bevy Web 客户端会直接加载该 `.ttf` 文件，因此在那里自定义字体会立即显示，在本地预览中也是如此。
* **不支持移动端**。Decentraland 移动应用会忽略 `fontSrc` 并渲染回退字体 `字体`。请选择一个看起来仍然合适的回退字体，不要依赖自定义字体来让文本适配固定空间。参见 [移动端缺失的功能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/missing-features/).
* **字体文件最大只能为 16 MB**。更大的文件会被拒绝，并使用回退字体 `字体` 。轮廓极其复杂的字体也可能被拒绝。
* **字符**：拉丁字符（ASCII 和 Latin-1）会预渲染。字体包含的任何其他字符，例如西里尔文或中文，都会在它们首次出现在场景中时渲染，这可能会造成短暂卡顿。字体不包含的字符会使用内置无衬线字体绘制。
* **一个文件对应一个字重**。一个 `.ttf` 文件只包含一种字重和样式。要同时有粗体标题和常规正文，请添加两个文件，并让每段文本分别指向各自的文件。请使用静态字体文件：如果某个字体家族只提供可变字体，请使用其静态实例。
* **字体是共享的**：所有使用同一 `fontSrc` 的文本实体都会共享同一份字体副本，因此在整个场景中重复使用同一字体不会比只用一次更昂贵。优先选择少量字体，而不是很多不同字体，因为每种字体都有自己的纹理。每个字体文件都会额外增加约 700 KB 的优化下载量。

当桌面客户端无法使用自定义字体时，它会在 `SDK_FONTS` 控制台的类别中记录一条消息，说明文件名以及原因。例如， `font_src "https://example.com/font.ttf" 不能是外部 URL`，或者该文件并不属于场景内容，或者场景尚未为其转换字体。这些消息默认是隐藏的。要显示它们，请输入 `/log-matrix enable SDK_FONTS Warning` 和 `/log-matrix enable SDK_FONTS Log` 到聊天中，并使用以下命令重新加载场景 `/reload`，然后使用以下命令打开日志文件夹 `/logs`。如果字体尚未转换，请检查该文件是否为 TrueType `.ttf` 且小于 16 MB，然后再次发布。

## 文本对齐和内边距属性

该 `TextShape` 组件会创建一个带有尺寸、内边距等属性的文本框。

* `文本对齐`：从 `TextAlignMode` 枚举中选择一个值。可选值包括垂直（*上*, *中间*, *下*）与水平（*左*, *居中*, *右*）对齐的所有组合。
* `宽度`: *数字*。文本框的宽度。
* `高度`: *数字*。文本框的高度。
* `上内边距`: *数字*。文本与文本框边框之间的间距。
* `右内边距`: *数字*。文本与文本框边框之间的间距。
* `下内边距`: *数字*。文本与文本框边框之间的间距。
* `左内边距`: *数字*。文本与文本框边框之间的间距。


> [!NOTE]
> **💡 提示**：如果文本要悬浮在空间中，最好添加一个 [`Billboard` 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#face-the-player) ，这样文本就会旋转，始终朝向玩家并保持可读。


## 文本阴影和轮廓属性

文本默认没有阴影，但你可以设置以下值来为其添加类似阴影的效果。

* `shadowBlur`: *数字*
* `shadowOffsetX`: *数字*
* `shadowOffsetY`: *数字*
* `shadowColor`: *Color3* 对象。 *Color3* 对象会存储一个 *RGB* 颜色，由 0 到 1 的三个数字表示。

```ts
TextShape.create(sign, {
	text: 'Text with shadow',
	shadowColor: { r: 1, g: 0, b: 0 },
	shadowOffsetY: 1,
	shadowOffsetX: -1,
})
```

文本中的字母也可以在其轮廓周围拥有不同颜色的描边。

* `outlineWidth`: *数字*。文本轮廓在所有方向上的宽度，取值为 0 到 1。默认情况下 *0*，因此它不可见。
* `outlineColor`: *Color3* 对象。 *Color3* 对象会存储一个 *RGB* 颜色，由 0 到 1 的三个数字表示。

## 多行

如果你希望文本跨越多行，请使用 `\n` 作为字符串的一部分。以下示例有两行彼此独立的文本：

```ts
TextShape.create(sign, {
	text: 'This is one line. \nThis is another line',
})
```

你还可以设置以下与多行文本相关的属性：

* `行数`: *数字*。文本框最多容纳多少行文本。如果未设置，则行数不受限制。 `文本换行` 属性必须为 *true* 才能使用多行。
* `行间距`: *数字*。每一行之间的间距。

## 文本朝向哪个方向

A `TextShape` 从其本地 **-Z** 一侧可正确阅读。若你创建一个且不旋转它，站在实体 -Z 一侧的玩家就可以阅读它。

因此，要给 **+Z** 面的模型添加标签，将文本作为模型的子级，并在 **+Z** 方向上稍微偏移，以免它嵌入网格，然后绕 Y 轴旋转 180 度：

```ts
// 附着在模型 +Z 面上的标签
import { engine, TextShape, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const label = engine.addEntity()

Transform.create(label, {
	parent: myModel,
	// 在 +Z 方向上做一个小偏移，以使文本离开模型表面
	position: Vector3.create(0, 1, 0.01),
	rotation: Quaternion.fromEulerDegrees(0, 180, 0),
})

TextShape.create(label, { text: 'Front' })
```


> [!NOTE]
> **💡 提示**：如果文本应当从玩家站立的任何位置都能读到，请添加一个 [Billboard](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#face-the-player) ，而不是手动旋转它。


## 将文本按列排版

在默认字体中，空格和制表符无法将列对齐，因为它们的字符宽度不同。用空格给记分板填充会得到参差不齐的列，只要某个名字或数字长度变化，列就会立刻错位。

要构建表格，请为每一列使用一个 `TextShape` 。为每列创建自己的实体，放置它，并将行写入其 `text` 字段中，并用 `\n`分隔。这样每一行在所有列中都会处于相同的高度。

```ts
// 一个双列表分数板：一个 TextShape 显示姓名，一个显示分数
const board = engine.addEntity()
Transform.create(board, { position: Vector3.create(8, 2, 8) })

const names = engine.addEntity()
Transform.create(names, { parent: board, position: Vector3.create(-1, 0, 0) })
TextShape.create(names, {
	text: 'Ana\nBruno\nChiara',
	textAlign: TextAlignMode.TAM_MIDDLE_LEFT,
	fontSize: 2,
})

const scores = engine.addEntity()
Transform.create(scores, { parent: board, position: Vector3.create(1, 0, 0) })
TextShape.create(scores, {
	text: '120\n85\n9',
	textAlign: TextAlignMode.TAM_MIDDLE_RIGHT,
	fontSize: 2,
})
```

将每列朝向列之间的空隙进行对齐： `TAM_MIDDLE_LEFT` 用于左列， `TAM_MIDDLE_RIGHT` 用于右列。数字采用右对齐效果最好，这样数字会对齐。

要更新某一行，请重写每一列的整个 `text` 字符串。保持每一列中的行数一致，否则各行就会无法对应。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
