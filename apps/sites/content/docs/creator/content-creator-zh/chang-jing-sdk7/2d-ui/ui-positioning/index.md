> 设置 UI 实体的位置、缩放、内边距和其他属性。

# UI 定位

对于各种 UI 内容，请使用 `uiTransform` 组件来设置大小、位置以及与实体对齐相关的其他属性。

该 `uiTransform` 组件在屏幕的 2D 空间中工作，非常类似于 `Transform` 组件在场景的 3D 空间中工作。

***ui.tsx 文件：***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '200px',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
		}}
		uiBackground={{ color: Color4.Green() }}
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
> **📔 注意**：本页下方的所有示例都假设你有一个 `.ts` 类似于上面的，正在运行 `ReactEcsRenderer.setUiRenderer()` 函数。


## 定位属性

UI 实体的对齐基于 Flexbox 对齐模型。这是一个非常强大的模型，可用于在大小可能变化的弹窗中动态组织嵌套实体。


> [!NOTE]
> **💡 提示**：Decentraland 的 UI 实现基于 [Yoga](https://yogalayout.com/docs/)。阅读 [这篇文章](https://www.joshwcomeau.com/css/interactive-guide-to-flexbox/) 可非常易懂且深入地了解 Flexbox 中可用的属性。


### 实体大小

使用 `width` 和 `height` 来设置实体的大小。支持以下几种值：

* `auto`：大小会自动适应内部内容。这对长度可能变化的文本非常方便。将值写为 "auto"。
* **百分比**：作为父级尺寸的百分比。将值写为以 "%" 结尾的字符串，例如 `10 %`.
* **像素**：将值写为数字。
* **屏幕宽度或高度**：可使用 vw（视口宽度）和 vh（视口高度）来表示运行 Decentraland 的窗口整体大小的一部分。例如 `10vw` 表示窗口宽度的 10%， `25vh` 表示窗口高度的 25%。

请注意，这些属性会影响 **default** 该项目的大小，即在进行任何 flex grow 和 flex shrink 计算之前的大小。最终大小的解释可能会根据父实体的大小以及所设置的 Flexbox 属性而有所不同。


> [!WARNING]
> **📔 注意**：在同时支持数字和字符串的属性中，要以像素设置值，请写数字。要将这些字段设置为父级尺寸的百分比，请将值写为以 "%" 结尾的字符串，例如 `10 %`。你也可以通过让字符串以 `px`，例如 `200px`.
> 
> * 当值以百分比表示时，它们始终相对于父级容器。你的 UI 的顶层实体也有一个父级：渲染器会将其放置在由 [`screenInset`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area)所选定的区域中，默认情况下该区域是设备安全区。因此，根级 `100%` 占该区域的 100%——在比屏幕更窄的手机上如此，在桌面上二者重合。传入 `screenInset: 'none'` 如果你需要按整个屏幕来计算百分比。
> * 如果值以像素表示，它们不会受父级缩放影响，但也不是 **不** 原始屏幕像素：它们会乘以从 [虚拟屏幕](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-virtual-scale)派生的 UI 缩放因子，而该虚拟屏幕默认处于启用状态。 `width: 200` 表示“在大小与虚拟屏幕相同的屏幕上为 200 px”，并会在其他屏幕上按比例放大或缩小。
> * 如果值以 `vh` 或 `vw`表示，它们是整个窗口的百分比，不受父级缩放、虚拟屏幕或 `screenInset`.
> 
> 对于 `auto` width/height 的影响，以下规则适用：
> 
> * 将 width/height 设为“auto”的 UiTransform 应具有 `alignSelf`: `“center”`/`“flex-start”`/`“flex-end”` 或 `positionType: “absolute”`
> * 如果子元素的 UiTransform 使用 `positionType: “absolute”`，父级不会适应其大小/位置
> * 如果子元素的 UiTransform 使用任何位置覆盖，父级不会适应其大小/位置


还可以使用以下其他属性以更高级的方式调整大小：

* `maxWidth` 和 `maxHeight`: *number* 或字符串（如 height 和 width）。实体可具有的最大大小。
* `minWidth` 和 `minHeight`: *number* 或字符串（如 height 和 width）。实体可具有的最小大小。如果父级太小，无法容纳实体的最小大小，它们将溢出父级。
* `flexBasis`：这是一个与轴无关的方式，用于提供沿主轴的项目默认大小。如果父级是 flex direction 为 row 的容器，那么为子元素设置 flex basis 类似于设置该子元素的宽度；如果父级是 flex direction 为 column 的容器，则类似于设置子元素的高度。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			alignSelf: 'center',
			alignContent: 'center',
			width: '80%',
			height: '30%',
			minWidth: 300,
			maxWidth: 2500,
			margin: { left: '10%', right: '10%' },
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```

### 排列子实体

默认情况下，子实体相对于其父级的左上角定位。你可以使用诸如 `justifyContent` 和 `alignItems` 来改变这种行为。


> [!NOTE]
> **💡 提示**：任何引用 *content* 的属性都指的是沿主轴排列的实体（由 `flexDirection`决定）。任何引用


* `flexDirection`：Flex 方向控制节点子元素的布局方向。这也称为主轴。主轴是子元素布局的方向。交叉轴是与主轴垂直的轴，或用于布局换行的轴。其值取自 `FlexDirectionType` 类型。可用选项如下：
  * `row` （默认）
  * `row-reverse`
  * `column`
  * `column-reverse`
* `justifyContent`：此属性描述如何在容器的主轴内对齐子元素。例如，你可以使用该属性将子元素在设置为 row 的容器中水平居中，或在设置为 column 的容器中垂直居中。该属性的值必须来自 `flexDirection` 设置为 row 或在 `flexDirection` 设置为 column 的容器中垂直居中。该属性的值必须来自 `JustifyType` 类型。可能的值有：
  * `flex-start` （默认）：将容器的子元素对齐到容器主轴的起始处。
  * `flex-end`：将容器的子元素对齐到容器主轴的末尾。
  * `center`：将容器的子元素在容器主轴上居中对齐。
  * `space-between`：在容器的主轴上均匀分布子元素，并在子元素之间分配剩余空间。
  * `space-around`：在容器的主轴上均匀分布子元素，并在子元素周围分配剩余空间。与 space-between 相比，使用 space-around 会使空间分布到第一个子元素的开头和最后一个子元素的末尾。
  * `space-evenly`：在容器的主轴上均匀分布子元素，并分配剩余空间，使子元素之间以及子元素与容器边缘之间的间隙都相等。
* `alignItems`：描述如何沿容器的交叉轴对齐子元素。align items 与 justify content 很相似，但它不是应用于主轴，而是应用于交叉轴。此属性需要来自 `AlignType` 类型。可用选项如下：
  * `stretch`： （默认）将容器的子元素拉伸以匹配容器交叉轴的高度。
  * `flex-start`：将容器的子元素对齐到容器交叉轴的起始处。
  * `flex-end`：将容器的子元素对齐到容器交叉轴的末尾。
  * `center`：将容器的子元素在容器交叉轴上居中对齐。
  * `baseline`：沿共同基线对齐容器的子元素。单个子元素可以被设为其父级的参考基线。
* `alignSelf`：align self 的选项和效果与 `alignItems` 相同，但不同于影响容器内的子元素，你可以将此属性应用于单个子元素，以更改其在父级中的对齐方式。align self 会覆盖父级通过 align items 设置的任何选项。其值取自 `AlignType`，请参见 `alignItems` 上文中这些选项的详细信息。
* `alignContent`：align content 定义了沿交叉轴的行分布方式。只有当项目使用 `flexWrap`换行到多行时才会生效。其值取自 `AlignType` 类型。可用选项如下：
  * `flex-start`：（默认）将换行后的行对齐到容器交叉轴的起始处。
  * `flex-end`：将换行后的行对齐到容器交叉轴的末尾。
  * `stretch`：拉伸换行后的行以匹配容器交叉轴的高度。
  * `center`：将换行后的行在容器交叉轴上居中对齐。
  * `space-between`：在容器的主轴上均匀分布换行后的行，并在各行之间分配剩余空间。
  * `space-around`：在容器的主轴上均匀分布换行后的行，并在各行周围分配剩余空间。与 space-between 相比，使用 space-around 会使空间分布到第一行的开头和最后一行的末尾。
* `flexGrow`：此属性描述容器内的任何剩余空间应如何沿主轴在其子元素之间分配。在布局完成后，容器会根据其子元素指定的 flex grow 值分配任何剩余空间。flex grow 接受任何大于等于 0 的浮点值，默认值为 0。容器会按照子元素的 flex grow 值加权分配剩余空间。
* `flexShrink`：描述在子元素总大小在主轴上超过容器大小时，如何沿主轴缩小子元素。flex shrink 与 flex grow 非常相似，如果将任何溢出的大小视为负的剩余空间，也可以用同样的方式理解。这两个属性还能很好地协同工作，使子元素能够按需放大和缩小。flex shrink 接受任何大于等于 0 的浮点值，默认值为 1。容器会按照子元素的 flex shrink 值加权缩小其子元素。
* `overflow`：决定当实体的子元素大小超出其父级时会发生什么。它使用来自 `OverflowType` 类型的值。
  * `hidden`：超出的实体会被隐藏。
  * `visible`：超出的实体会突破父级的边距。
  * `scroll`：该区域变为可滚动，允许玩家滚动查看溢出的内容。参见 [可滚动容器](#scrollable-containers) 了解详细信息。
* `flexWrap`：flex wrap 属性设置在容器上，并控制当子元素沿主轴超出容器大小时会发生什么。默认情况下，子元素会在需要时沿主轴换行到多行。如果通过 `nowrap`禁用换行，子元素会被强制放入单行（这可能会压缩实体）。wrap reverse 的行为与 wrap 相同，但行顺序会反转。此属性的值取自 `FlexWrapType` 类型的值。
  * `wrap`
  * `nowrap`
  * `wrap-reverse`

### 外边距和内边距

* `margin`：此属性会影响节点外部周围的间距。带有 margin 的节点会从其父级边界处偏移，同时也会影响任何兄弟节点的位置。如果父级是自动尺寸，则节点的 margin 会计入父级的总大小。在实体与其父级的边距之间设置空间。期望值是一个包含以下属性的对象 `顶部`, `left`, `底部`，和 `right`.
* `padding`：此属性会影响其所应用节点的大小。在 Yoga 中，padding 的行为就像设置了 box-sizing: border-box; 一样。也就是说，如果实体已经显式设置了大小，padding 不会增加实体的总大小。对于自动尺寸节点，padding 会增加节点的大小，并偏移任何子元素的位置。期望值是一个包含以下属性的对象 `顶部`, `left`, `底部`，和 `right`.

### 微调位置

在 Flexbox 中，实体的位置主要取决于它们的父子关系，以及父级和子级上设置了哪些布局属性。你通常完全不需要设置 `position` 属性。但如果你确实想对其进行调整，或者完全覆盖 Flexbox 的正常流并设置绝对位置，以下是相关属性：

* `positionType`：定义实体的定位方式。它使用来自 `PositionType` 枚举中的一个值。
  * `relative`：（默认）默认情况下，实体采用相对定位。这意味着实体会按照布局的正常流进行定位，然后再根据 `顶部`, `right`, `底部`，和 `left`的值相对于该位置进行偏移。该偏移不会影响任何兄弟节点或父节点的位置。
  * `absolute`：当采用绝对定位时，实体不参与正常的布局流。它会独立于兄弟节点进行布局。其位置根据 `顶部`, `right`, `底部`，和 `left` 权重值播放。
* `position`：位置值 `顶部`, `right`, `底部`，和 `left` 的行为会根据 `positionType`而有所不同。对于相对实体，它们会按指定方向偏移实体的位置。对于绝对实体，这些属性则指定实体某一边相对于父级同一边的偏移量。期望值是一个包含以下属性的对象 `顶部`, `left`, `底部`，和 `right`.


> [!WARNING]
> **📔 注意** ： `顶部` 或 `left` 的正值表示从父级同一边向内测量的距离。例如，要将组件定位为相对于父级在顶部和左侧保留 20 像素的边距，请设置 `position` 设置为 `{ top: 20, left: 20 }`.


### 可见性

* `显示`：决定实体是否可见。要使实体不可见，请设置 `显示` 设置为 `none`.

### Z 索引

该 `zIndex` 的属性 `UiEntity` 决定实体的渲染顺序。具有更高 `zIndex` 的实体会渲染在较低 `zIndex`之上。默认 `zIndex` 为 0。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			zIndex: 4
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```


> [!WARNING]
> **📔 注意** ： `zIndex` 属性只会对直接兄弟元素的相对顺序进行排序，它不能用于将实体渲染在布局树其他部分之上。用 html/CSS 的术语来说，每个 DCL UI 元素都会创建一个新的 [层叠上下文](https://web.dev/learn/css/z-index#stacking_context)。要对通过 `addUiRenderer()` 添加的独立 UI 模块相互排序，请使用 `zIndex` 的 [渲染器选项](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#stacking-order-between-ui-modules) 代替。
> 
> 默认的 Decentraland UI，包括地图、聊天等，始终渲染在所有其他 UI 元素之上。


## 可滚动容器

当 UI 实体包含的内容比其分配的大小更多时，你可以通过设置 `overflow` 设置为 `scroll` 来使该区域可滚动 `uiTransform`实体的

。然后玩家可以通过拖动或使用鼠标滚轮来滚动查看内容。 `width` 和 `height`）的固定大小，并且子元素必须超过该大小。

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const scrollableMenu = () => (
	<UiEntity
		uiTransform={{
			width: 300,
			height: 400,
			overflow: 'scroll',
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#1a1a1a') }}
	>
		{/* 这些子元素超过了父级 400px 的高度，使该区域可滚动 */}
		<Label value="项目 1" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 2" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 3" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 4" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 5" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 6" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="项目 7" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
	</UiEntity>
)
```

这对于构建长列表、库存、聊天记录、排行榜或任何内容可能超出屏幕显示范围的面板都很有用。

你也可以将可滚动容器嵌套到其他 UI 布局中。例如，一个带有固定标题和可滚动主体的对话框弹窗：

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const dialogWithScroll = () => (
	<UiEntity
		uiTransform={{
			width: 400,
			height: 500,
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#2a2a2a') }}
	>
		{/* 固定标题 */}
		<Label
			value="排行榜"
			fontSize={22}
			uiTransform={{ width: '100%', height: 60 }}
		/>

		{/* 可滚动主体 */}
		<UiEntity
			uiTransform={{
				width: '100%',
				flexGrow: 1,
				overflow: 'scroll',
				flexDirection: 'column',
			}}
		>
			<Label value="1. Alice - 9500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="2. Bob - 8200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="3. Charlie - 7800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="4. Diana - 6100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="5. Eve - 5500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="6. Frank - 4900" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="7. Grace - 4200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="8. Hank - 3800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="9. Ivy - 3100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="10. Jack - 2700" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
		</UiEntity>
	</UiEntity>
)
```


> [!NOTE]
> **💡 提示**：使用 `flexGrow: 1` 在可滚动实体上，使其填满父级中剩余的空间，这样当其他兄弟元素（如标题或页脚）改变大小时，它也能自适应。


## 响应式 UI 大小

不同屏幕尺寸的玩家看到的 UI 布局可能不同。像素值会按 [虚拟屏幕](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) 为你缩放，因此同一 UI 可在不同分辨率下保持比例—— **你无需自己计算缩放因子**，否则会重复应用缩放。


> [!WARNING]
> **📔 注意**: `devicePixelRatio` 不会参与 UI 布局。它只是一个显示密度提示——用于在 1x、2x 或 3x 的纹理版本之间进行选择——仅此而已。如果你的场景是按较早版本的 SDK 调整大小的，那么在高密度（Retina 和移动设备）屏幕上，按像素尺寸设置的 UI 可能会渲染得大 2 到 3 倍，请重新检查任何手工微调过的内容。



> [!CAUTION]
> **📔 移除你自己的缩放因子。** 如果你的场景将其大小乘以一个从 `UiCanvasInformation` 计算出的因子——通常是 `Math.min(width / 1920, height / 1080)` ——请移除该乘数。它与 SDK 现在默认应用的因子相同，因此两者同时保留会使 UI 随屏幕尺寸呈平方级增长。如果你更愿意只保留自己的因子，请使用 `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.


`UiCanvasInformation`，它默认添加到场景的根实体上，仍然是处理缩放无法表达的布局决策的正确工具——例如在窄屏上采用不同的对话框布局、从 `devicePixelRatio`中选择纹理分辨率，或者自行读取安全区域。它不是用于尺寸设定的正确工具。

该 `UiCanvasInformation` 组件包含以下信息：

* `height`：画布高度（像素）
* `width`：画布宽度（像素）
* `devicePixelRatio`：设备上的物理像素分辨率与画布像素之间的比率。可作为显示密度提示，例如用于在 1x、2x 或 3x 版本的纹理之间进行选择。
* `interactableArea`: 一个 `BorderRect` 对象，详细说明为场景 UI 元素指定的区域。此对象包含以下值： `顶部`, `底部`, `left` 和 `right`，这些值分别是屏幕相应边缘被 explorer UI 占用的像素数。
* `screenInsetArea`: 一个 `BorderRect` 对象，详细说明由设备或平台 UI 保留的屏幕内嵌区域（安全边距），例如移动设备上的刘海、状态栏、Home 指示条或圆角。此对象包含以下值： `顶部`, `底部`, `left` 和 `right`，这些值分别是该屏幕边缘保留的像素数。在桌面端，这通常是 `0` 四边均为 0。


> [!WARNING]
> **📔 注意** 不同的 Decentraland explorer 对这些值会有所不同，因为平台的全局 UI 可能不同，而且当用户展开或隐藏不同的全局 UI 菜单时，这些值可能会动态变化。


```ts
import { engine, UiCanvasInformation } from "@dcl/sdk/ecs"

export function Main() {
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  if (!canvas) return
  console.log("画布尺寸：", canvas.width, canvas.height)
}
```

关于 UI 尺寸的其他一些最佳实践：

* 如果任何 UI 元素的宽度或高度是动态的，最好也使用 `maxWidth`, `minWidth`, `maxHeight`，和 `minHeight` 参数，以确保它们保持在合理的值范围内。
* 数值型字体大小是虚拟像素值，会像其他值一样按比例缩放。如果你希望尺寸改为相对于画布来衡量，使其不受虚拟屏幕影响，请传入一个 `vw`/`vh` 字符串——参见 [响应式文本大小](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#responsive-text-size)


> [!NOTE]
> **💡 提示**：有关 UI 尺寸的可运行示例，请参见 [`81,-2-ui-screen-inset-area`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) 测试场景，它会运行这三种 `screenInset` 模式（`'none'`, `'device'`, `'interactable'`）作为三个共存的渲染器，并打印实时 `screenInsetArea` 和 `interactableArea` 值，以及 [`76,-10-UiCanvasInformation`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/76,-10-UiCanvasInformation)，它会读取 `UiCanvasInformation` 每一帧以响应式方式调整 UI 大小。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
