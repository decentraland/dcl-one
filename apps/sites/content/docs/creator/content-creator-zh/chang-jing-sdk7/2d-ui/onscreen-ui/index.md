> 了解如何为场景中的玩家创建 UI。例如，这可用于显示与游戏相关的信息。

# 屏幕 UI

你可以为你的场景构建一个 UI，将其显示在屏幕固定的 2D 空间中，而不是 3D 世界空间中。

当玩家站在场景的 LAND 地块内时，UI 元素才可见，因为相邻场景可能有自己的 UI 要显示。UI 的部分内容也可以在世界空间中发生某些事件时被触发打开，例如玩家点击了某个特定位置。

通过定义嵌套的 `UiEntity` 对象结构来构建 UI。用于 UI 的语法与 [React](https://reactjs.org/) （一个非常流行的、基于 JavaScript 的网页 UI 构建库）非常相似。


> [!WARNING]
> **📔 注**: 你只能在具有 `.tsx` 扩展名的文件中定义 UI 语法。 `.tsx` 文件支持 `.ts` 文件支持的全部内容，以及 UI 语法。我们建议创建一个 `ui.tsx` 文件，并在那里定义你的 UI。记得从 `index.ts` 中使用 `ReactEcsRenderer.setUiRenderer(yourUiMethodName)`调用你的 UI 渲染方法，见下例。


简单的静态元素 UI 看起来很像 HTML，但当你添加会响应状态变化的动态元素时，你就可以做出更强大的东西。

Decentraland 默认的浏览器 UI 包括聊天窗口、小地图和其他元素。这些 UI 元素始终显示在最顶层，位于任何场景特定 UI 之上。因此，如果你的场景中有与这些元素占据相同屏幕空间的 UI 元素，它们会被遮挡。

参见 [UX 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/) ，获取有关如何设计 UI 外观和体验的提示。


> [!NOTE]
> **📱 移动端设计**：设备会为刘海、状态栏、主页指示条和圆角预留屏幕空间，客户端也会在画布的一部分上绘制自己的控件。你无需手动测量——请使用下面的 [`screenInset`](#screen-inset-area) 来选择你的 UI 所放置的区域。它默认使用设备安全区域，因此无需你额外处理即可避开硬件边距。在发布之前，请查看 [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/) 以及 [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/).


当玩家点击屏幕右下角的 *关闭 UI* 按钮时，所有 UI 元素都会被隐藏。

## 渲染 UI

要在你的场景中显示 UI，请使用 `ReactEcsRenderer.setUiRenderer()` 函数，传入一个有效的实体结构，该结构在 `.tsx` 文件中描述。

每个实体都定义为一个类似 HTML 的节点，并为其每个组件提供属性。

***ui.tsx 文件：***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      margin: { top: '35px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Red() }}
  />
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

你也可以通过在一个 `.tsx` 文件中描述。

***ui.tsx 文件：***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUI() {
  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity
      uiTransform={{
        width: 700,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiBackground={{ color: Color4.Red() }}
    />
  ), { virtualWidth: 1920, virtualHeight: 1080 })
}
```

***index.ts 文件：***

```ts
import { setupUI } from './ui'

export function main() {
    setupUI()
}
```


> [!WARNING]
> **📔 注**：你的所有 UI 元素都需要嵌套在同一个结构中，并在结构根部只有一个单一父级。你只能在场景中调用 `ReactEcsRenderer.setUiRenderer()` 一次。


## UI 实体

UI 中的每个元素都必须定义为单独的 `UiEntity`，无论它是图片、文本、背景、不可见的对齐框，等等。就像场景的 3D 空间一样，每个 `UiEntity` 都有自己的组件来赋予它位置、颜色等。

类似 React 的语法允许你将每个组件作为 `UiEntity`中的一个属性来指定，这使代码更简短、更易读。

在 `UiEntity` 中使用的组件与普通实体中的不同。你不能把 UI 组件应用到普通实体上，也不能把普通组件应用到 UI 实体上。

以下组件可用于 `UiEntity`:

* `uiTransform`
* `uiBackground`
* `uiText`
* `onMouseDown`
* `onMouseUp`
* `onMouseEnter`
* `onMouseLeave`
* [`uiInputBinding`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding/) — 在元素被按下时保持输入动作，以构建自定义控制

与 HTML 标签一样，你可以将组件定义为自闭合，或将一个嵌套在另一个内部。


> [!CAUTION]
> **警告：** 请保持全屏布局包装器不带指针处理器。向 `onMouse` 处理器，或 `pointerFilter: 'block'`，添加到一个 `UiEntity` 大小为 `100%` 由 `100%` 决定的组件上，会让它捕获屏幕上的每一次点击，使你所有其他 UI 和整个 3D 世界都无法被点击。请把处理器放在需要它们的面板或按钮上。参见 [指针阻挡](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_button_events/#pointer-blocking).


***ui.tsx 文件：***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  // 父实体
  <UiEntity
    uiTransform={{
      width: 200,
      height: 200,
      margin: { top: '250px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Blue() }}
  >
    {/* 自闭合子实体 */}
    <UiEntity
      uiTransform={{
        width: 400,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiText={{ value: `Hello world!`, fontSize: 40 }}
    />
    {/* 父实体的结束语句 */}
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

一个 UI 模块的定义只能有一个顶层父实体。你可以定义任意多个其他实体，但它们都必须嵌入到顶部只有一个父级的结构中。

## 屏幕虚拟缩放

为 UI 设置虚拟宽度和高度。这样可以确保你的 UI 在不同屏幕尺寸下看起来相同，而不受实际像素屏幕尺寸的影响。

```ts
export function setupUi() {
    ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

如果你将虚拟宽度设为 1920，虚拟高度设为 1080，UI 将按比例缩放以适应屏幕尺寸。如果屏幕是 1920x1080，UI 将以与虚拟尺寸相同的大小显示。如果屏幕更大或更小，任何像素值都会按比例缩放以适应虚拟尺寸。例如，如果屏幕是 3840x2160，那么一个定义为宽度 100 像素的项目将显示为 200 个实际像素。

实际用于乘以像素值的 UI 缩放因子计算公式是 [`Math.min(realWidth / virtualWidth, realHeight / virtualHeight)`](https://github.com/decentraland/js-sdk-toolchain/blob/main/packages/%40dcl/react-ecs/src/system.ts).

#### 全屏图像比你的虚拟尺寸更高

缩放因子会将你的参考分辨率保持在 **内** ，因此在屏幕形状与虚拟尺寸不同的屏幕上，多出来的空间仍然属于 UI。百分比不会缩放，所以根元素设置为 `width: '100%'`, `height: '100%'` 会覆盖整个屏幕，这在一个方向上可能会超过你的虚拟尺寸。

在默认 `1920x1080`的 16:10 屏幕上，缩放因子来自宽度，因此那个全屏根元素测量为 **1920x1200** 个虚拟像素。在更宽的屏幕上，它则会比 1920 更宽。

因此，对于必须填满屏幕的背景图：

* 导出 **大于 1920x1080 的素材** 并让它裁切，就像 CSS 的 `cover` 适配方式那样。尺寸恰好为 1920x1080 的素材在更高的屏幕上会留下黑边。
* 请将任何必须保持可见的内容（标志、标题、按钮）放在居中的 1920x1080 区域内，并将其外部的所有内容视为可以安全丢失。

### 默认虚拟尺寸

虚拟尺寸是可选的，但它的两个维度必须同时提供：要么同时传入，要么都不传。当你不传入其中一个时，会应用平台默认值：

| 平台      | 默认虚拟尺寸      |
| ------- | ----------- |
| 移动端     | `1600x720`  |
| 桌面端和网页端 | `1920x1080` |

这意味着，即使你完全不传任何选项，UI 中的像素值也始终会相对于一个参考分辨率进行缩放。即使场景只使用 `addUiRenderer()` 而从不调用 `setUiRenderer()`.

也是如此。

* **三个特殊情况：**&#x53D6;消缩放 `0` ：传入一个无效尺寸——任何

  ```ts
  或更小的值——即可完全禁用虚拟屏幕。此时像素值会直接作为原始画布像素使用，不进行任何缩放。这是文档中关闭虚拟屏幕的方式，因此不会记录日志。
  ```
* **不完整的尺寸**：只给出两个维度中的一个也是无效的，因此它会像上面的情况一样禁用虚拟屏幕——平台默认值不会接管。 **不** 。与上面的取消方式不同，这种情况会被报告：只给出一半尺寸是错误，而不是有意的选择，因此 SDK 会记录 `虚拟屏幕尺寸不完整（…）：需要两个维度，因此虚拟屏幕已禁用，并且未应用任何 UI 缩放。` 每种尺寸只记录一次。请同时传入两个维度，或者都不传。

  ```ts
  // 不要这样做——不会应用任何缩放
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920 })
  ```

  值得在主渲染器上再次检查，因为那里的不完整尺寸同样是一个影响整个场景的开关：一个提到 `setUiRenderer()` 的调用 *任一* 维度都会优先于任何 `addUiRenderer()`，因此上面的示例会为整个场景禁用虚拟屏幕，并丢弃传给附加渲染器的有效尺寸。
* **移动端上的 16:9 尺寸**：手机屏幕比 16:9 宽得多，因此 16:9 的虚拟画布会给 UI 加上黑边。如果你传入 16:9 的尺寸（`1920x1080`, `1280x720`，…）且场景运行在移动端，它会被覆盖为 `1600x720`，并且控制台会记录一次消息。移动端上的非 16:9 尺寸，以及桌面端或网页端上的所有有效尺寸，都会按原样保留。


> [!NOTE]
> **📔 注**： `vw` 和 `vh` 单位与虚拟屏幕无关： `1vw` 是画布宽度的 1%，而 `1vh` 是画布高度的 1%，与 CSS 中完全相同。它们也会忽略 [`screenInset`](#screen-inset-area)，因此在默认的 `'device'` inset 中， `width: '100vw'` 会比 `width: '100%'` 更宽——前者覆盖整个屏幕，后者填满内嵌区域。



> [!NOTE]
> **📱 注意**：平台检测是异步完成的。在最初几帧中，SDK 还不知道自己正在移动端上运行，因此虚拟屏幕会先从 `1920x1080` 开始，并在检测完成后切换到 `1600x720` ——你可能会在加载时看到 UI 短暂重新缩放，并且 16:9 覆盖消息会在场景开始后片刻出现在控制台中。如果你希望从第一帧开始就有一个稳定的参考分辨率，请传入一个明确的非 16:9 虚拟尺寸。



> [!WARNING]
> **📔 本 SDK 版本之前编写的场景有哪些变化**
> 
> 有三项变更会影响现有场景。它们都不是可选启用的，因此一个完全不改动的场景在更新 SDK 版本后外观也会不同：
> 
> 1. **虚拟屏幕现在默认启用。** 过去，不传任何选项的场景会把像素值按原始画布像素布局。现在它会相对于 `1920x1080` (`1600x720` 进行缩放（移动端上则是相对于 `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.
> 2. **`screenInset` 默认为 `'device'`.** 你的 UI 现在被放置在设备安全区域内，因此在移动端上它会向内移动，而根级的 `100%` 不再是整个屏幕——全屏背景、遮罩和覆盖层会避开刘海和主页指示条。若要恢复之前的行为，请传入 `setUiRenderer(ui, { screenInset: 'none' })`。参见 [屏幕内嵌区域](#screen-inset-area).
> 3. **`devicePixelRatio` 不参与 UI 布局。** 现在在高密度（Retina 和移动端）屏幕上，按像素大小定义的 UI 会比之前大 2–3 倍。 **对此没有关闭选项** ——请重新检查任何手工微调过的尺寸，并移除场景自己计算出的任何缩放因子。


## 屏幕内嵌区域

屏幕并不能完全自由使用：手机会为刘海、状态栏、主页指示条和圆角预留空间，而每个浏览器都会在画布的一部分上绘制自己的 HUD（小地图、聊天等）。渲染器选项中可选的 `screenInset` 属性用于选择你的 UI 所放置的屏幕区域：

| 值                 | UI 所放置的区域                                                                                             |
| ----------------- | ----------------------------------------------------------------------------------------------------- |
| `'device'` *（默认）* | 设备安全区域，不包括刘海、状态栏和圆角。读取自 `UiCanvasInformation.screenInsetArea`.                                        |
| `'interactable'`  | 浏览器报告为不包含其自身 HUD（小地图、聊天等）的区域。读取自 `UiCanvasInformation.interactableArea`。它覆盖哪些部分取决于每个浏览器——请在你的目标平台上验证。 |
| `'none'`          | 整个屏幕，左上角从 `0,0` 开始。                                                                                   |

```ts
// UI 保持在刘海、状态栏和圆角之外——这是默认设置
ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })

// UI 保持在浏览器自己的 HUD 之外
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })

// UI 覆盖整个屏幕，边距由你自己处理
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'none' })
```

在桌面端，设备内嵌为零，因此 `'device'` 会让 UI 覆盖整个屏幕——这与 `'none'`的结果相同。该区域会在每个 tick 重新读取，因此 UI 会随着内嵌变化而跟随，例如在旋转或系统栏出现/隐藏时。


> [!WARNING]
> **📔 不要将 UI 锚定在左上角。** 那个角，以及整个左边缘，是浏览器绘制小地图、聊天以及移动端虚拟摇杆的地方。放在那里的场景 UI 虽然会渲染，但客户端自己的 UI 会绘制在其上方，因此会被遮住，而且按钮通常无法点击。请将 UI 锚定到右侧，或将其居中，或者使用 `screenInset: 'interactable'` ，让浏览器将其放置在远离自身 HUD 的区域。参见 [永远不要将 UI 锚定到左上角](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).



> [!WARNING]
> **📔 `'interactable'` 在桌面端并不是无操作。** 与设备内嵌不同，可交互区域 *不* 在桌面客户端上是零：它为自身 UI 预留了大约屏幕左侧 25% 的区域，因此 `screenInset: 'interactable'` 会将你的 UI 放在剩余的 75% 区域内。这正是该选项的目的，但这也意味着它会改变你的桌面布局——如果你只想在手机上启用它，请用 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) 进行分支处理。
> 
> **客户端支持**: `'interactable'` 需要浏览器报告该区域。桌面端支持该功能，移动端则从客户端版本 `1.12.1` 开始支持——在更旧的移动客户端中，该值会被报告为零，UI 会回退为覆盖整个屏幕。同一个 `1.12.1` 版本还会统一 `'device'` 区域在 Android 和 iOS 上的表现，因此把它视为依赖任一内嵌布局的下限。 `'none'` 在任何地方的行为都相同。



> [!WARNING]
> **📔 注**：不要将你的 UI 包裹在 [`ScreenInsetArea` 或 `InteractableArea`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/#wrap-part-of-your-ui-instead) 组件中，同时还保留渲染器上的匹配 `screenInset` 值——这样内嵌会被应用两次，把 UI 向内推移双倍边距。要么依赖 `screenInset`，要么将其设为 `'none'` ，然后自己放置包装器。


### 真实设备上的三个区域

下面的截图是在同一部手机上以同一场景渲染三次得到的，只是 `screenInset` 值不同。洋红色矩形是 UI 本身；轮廓是浏览器报告的区域——青色是整个画布，琥珀色是 `screenInsetArea`，绿色是 `interactableArea`.

<figure><img src="/docs/_remote/75b3a862ee39f3dbb1722d14.webp" alt="Scene UI covering the whole phone screen, including the notch strip and the areas under the client controls"><figcaption><p><code>screenInset: 'none'</code> ——UI 覆盖整个画布。它的角落位于刘海之下、且在客户端自己的控件后面。</p></figcaption></figure>

<figure><img src="/docs/_remote/b3f7d16c8434488f8b197eda.webp" alt="Scene UI inset to the device safe area, clear of the notch"><figcaption><p><code>screenInset: 'device'</code> （默认）——UI 被拉入设备安全区域，避开刘海和圆角。但它仍然会覆盖客户端的控件，因为这些控件不属于此区域。</p></figcaption></figure>

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> ——UI 被放置在客户端为场景 UI 指定的矩形区域内。在这里捕获的移动客户端 <code>1.12.1</code>上，这不包括左侧栏（个人资料、聊天、摇杆）；右下角的操作按钮会按设计绘制在该区域上方。</p></figcaption></figure>


> [!NOTE]
> **💡 提示**: `'interactable'` 会为你提供每个浏览器为场景 UI 指定的区域。它有意并不等同于“所有客户端控件都在其外部”——正如第三张截图所示，移动端操作按钮会故意绘制在该区域上方。你放在它们下面的任何内容仍然可以被你触达，但会争夺同样的点击，因此请记住 [拥挤的角落](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/#where-the-client-controls-live) ，并检查你的目标平台：该区域覆盖哪些部分由各浏览器决定，并可能在不同版本之间变化。


每个渲染器都会遵循自己的 `screenInset`，因此主 UI 和任何通过 `addUiRenderer()` 添加的 UI 可以同时使用不同的区域。与虚拟尺寸不同，这个值不会在各个渲染器之间共享。

## 多个 UI 模块

如果你的场景包含多个系统或模块，而它们各自定义了自己的 UI，你可以使用 `ReactEcsRenderer.addUiRenderer()`分别渲染每个 UI 模块。这在处理一个包含多个 UI 组件的复杂场景时尤其有用，或者在为一个 [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)定义 UI 时也很有帮助，因为它应该能够独立于场景其余代码中的内容而使用。

该 `ReactEcsRenderer.addUiRenderer()` 函数要求你提供一个实体作为 UI 的所有者。这个实体可以是任何实体，甚至可以是仅为了作为 UI 所有者而创建的虚拟实体。

```ts
export function setupUi() {

    // 创建一个虚拟实体作为 UI 的所有者
    const dummyEntity = engine.addEntity()

    // 将 UI 模块定义为一个返回 UI 模块数组的函数
    const uiComponent = () => [
      // 返回一个 UI 模块的函数，
      // 返回一个 UI 模块的函数
      // ...
    ]

    // 使用虚拟实体作为所有者来渲染 UI 模块
    ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent)
}
```

这段代码可以独立于场景中的任何其他 UI 代码存在。场景的其余部分可能包含一个 `ReactEcsRenderer.setUiRenderer()`，或者根本没有任何内容，而 UI 仍然会被渲染。

一个 `addUiRenderer()` 调用也可以像 `setUiRenderer()`一样包含虚拟宽度和高度。不过，如果场景中有一个也定义了虚拟宽度和高度的 `setUiRenderer()` 调用，那么 `addUiRenderer()` 调用的虚拟宽度和高度将被忽略。

```tsx
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

虚拟尺寸是一个场景范围内的单一值，其解析规则如下： `setUiRenderer()` 上的尺寸优先；否则，第一个提供了该值的 `addUiRenderer()` 调用获胜；如果没有渲染器提供该值，则适用 [平台默认值](#default-virtual-size) 。只包含 `screenInset` 的选项不算提供了尺寸。

该 [`screenInset`](#screen-inset-area) 的规则正好相反——它是按渲染器分别生效的，因此每个 UI 模块都可以位于不同的屏幕区域：

```tsx
// 这个小部件会避开浏览器 HUD，无论主 UI 使用什么区域
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { screenInset: 'interactable' })
```

该 UI 可以通过 `ReactEcsRenderer.removeUiRenderer(dummyEntity)` 移除。另外，如果拥有该 UI 的实体被销毁，该 UI 也会被移除。如果 `ReactEcsRenderer.addUiRenderer()` 再次针对同一实体调用，但使用不同的 UiRenderer，之前的会被清理，新的一会替换它，并保留其在堆叠顺序中的位置。再次调用它也是你更改已经渲染的 UI 模块选项的方式，例如其 `zIndex`.

### UI 模块之间的堆叠顺序

默认情况下，UI 模块会按它们首次出现的顺序堆叠，后出现的会位于前面。首次出现在同一帧中的模块——通常是场景第一帧之前注册的所有内容——会与主 UI 一起绘制，从 `setUiRenderer()` 背面开始，然后按添加顺序绘制其他模块。若要显式控制这一点，请在 `zIndex` 渲染器选项中传入一个更高的 `zIndex` 值：更高的 `0` 值的模块会显示在更低值模块前面，而不受它们出现顺序的影响。留在 `setUiRenderer()`默认值时保持默认顺序。该选项在

```tsx
// 虽然最先注册，但始终位于其他 UI 模块前面
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { zIndex: 10 })

// 主 UI 位于所有未设置更低值的模块后面
ReactEcsRenderer.setUiRenderer(mainUi, { zIndex: -10 })
```

这是与 `zIndex` 一个 `UiEntity`不同的设置， [它只对](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui-positioning/#z-index)UI 模块内的同级项排序。渲染器选项会让整个模块彼此排序，而每个模块内部的元素仍保持自己的 `zIndex`.

### 共享一个 setUiRenderer 语句

不必为每个 UI 模块都调用 `ReactEcsRenderer.addUiRenderer()` ，你可以调用 `ReactEcsRenderer.setUiRenderer()` 一次性传入一个 UI 模块数组，这些模块可以位于不同文件中。

```ts
const uiComponent = () => [
  // 返回一个 UI 模块的函数，
  // 返回一个 UI 模块的函数
  // ...
]

ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

下面是一个更完整的示例：

***ui.tsx 文件：***

```ts
export function UIModule1() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', bottom: '3%' },
      }}
    >
      <Label value="你好，世界！" fontSize={18} textAlign="middle-center" />
    </UiEntity>
  )
}

export function UIModule2() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', top: '3%' },
      }}
    >
      <Label
        value="这里还有更多 UI！"
        fontSize={18}
        textAlign="middle-center"
      />
    </UiEntity>
  )
}
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { UIModule1, UIModule2 } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(() => [
      UIModule1(),
      UIModule2(),
      // ...
      // 下面这行用于使用 DCL UI Toolkit 库
      // https://github.com/decentraland-scenes/dcl-ui-toolkit
      ui.render(),
    ], { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
