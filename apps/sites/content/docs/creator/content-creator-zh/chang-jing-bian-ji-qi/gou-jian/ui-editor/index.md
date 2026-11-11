> 在 Creator Hub 中以可视化方式在画布上设计你场景的屏幕 UI，而不是在代码中进行设计。

# UI 编辑器

UI 编辑器让你通过将控件拖到画布上来构建场景的屏幕界面，而不是手写布局代码。它会把真实文件写入你的场景，因此你在这里创建的任何内容都是普通的 SDK7 UI，你仍然可以继续用代码编辑它。

## 打开它

使用 **2D** / **3D** 编辑器左侧面板顶部的切换开关。 **3D** 就是你已经熟悉的场景画布。 **2D** 就是 UI 编辑器。

![2D 和 3D 选项卡切换，当前选中的是 3D。](/docs/_remote/4e696bf6f0bb7532d974ec02.webp)


> [!WARNING]
> **📔 注意**：UI 编辑器需要 `@dcl/sdk` 场景中使用 7.26.0 或更新版本。在较旧的场景中， **2D** 选项卡会显示 **UI 编辑器不可用** 提示，而不是画布，并带有一个 **更新 SDK** 按钮，它会为你升级场景，以及一个 **稍后再说** 按钮用于关闭提示。


Creator Hub 会记住你离开场景时处于 2D 还是 3D，并以相同方式重新打开。此设置按项目保存，绝不会随你的场景一起发布。

如果你使用的是 Bevy 场景渲染器，切换到 **2D** 会暂停正在运行的场景，而切回到 **3D** 如果它之前在运行，就会恢复运行。

## 制作你的第一个 UI

1. 切换到 **2D** 选项卡。
2. 点击 **新建 GUI**，可以点击空画布中央的按钮，或点击 **+** 旁边的 **GUIs** 左侧面板中的。GUI 是一个 UI 组件，保存在自己的文件中。
3. 从底部的组件面板拖一个控件到画布上。 **全屏** 是一个很好的首个控件，它会给你一个覆盖屏幕的根节点。然后把其他控件放到它里面。
4. 选中一个控件，并在右侧面板中设置其属性。

没有保存按钮。编辑器会在你编辑时将更改写入磁盘，工具栏左侧的徽标显示 **所有更改已保存**.

![2D 模式下的 UI 编辑器：左侧的 GUIs 和 Nodes 面板列出一个名为 MainUI 的 GUI，其中有一个包含 Label 和 Button 的 Container；中间的画布显示该容器，Button 处于选中状态并带有周围的缩放手柄；右侧的 Properties 选项卡显示该 Button 的位置和布局；底部是控件面板。](/docs/_remote/3958b77d4c214591522f3386.webp)

## 面板

* **左侧**: **GUIs**，你的 UI 组件列表，以及 **Nodes**，所选组件的树状结构。搜索框可同时筛选两者。悬停在某个节点上可锁定、隐藏或删除它。
* **右侧**：一个 **Properties** 选项卡用于所选节点，以及一个 **Logic** 选项卡。
* **底部**：控件面板。将卡片拖到画布上即可添加。
* **中间**：画布。拖动节点可移动它们，使用手柄可调整大小，并可用鼠标滚轮或拖动自由平移。

画布右下角有两组浮动控件：

* 该 **预览** 组用于切换移动端 HUD 辅助线和安全区域辅助线，并在 **桌面预览** 和 **移动预览**.
* 该 **缩放** 组有减号按钮和加号按钮，以及一个百分比输入框。点击百分比可输入 10 到 200 之间的精确缩放值，然后按 Enter 或点击其他位置应用，或按 Escape 取消。当缩放达到 200% 时，加号按钮会失效。

<div align="left"><img src="/docs/_remote/589a76a30cb6467ad0c864b3.webp" alt="UI 编辑器左侧面板，包含 2D 和 3D 选项卡、搜索框、包含 MainUI 的 GUIs 列表，以及显示 Container、Label 和 Button 的 Nodes 树，其中 Button 行显示锁定、隐藏和删除图标。" width="220"> <img src="/docs/_remote/e71a2f9dd042f80664595bb6.webp" alt="UI 编辑器中选中的 Button 的 Properties 选项卡，包含 Visibility、Interaction States、带有 Constraints、Position 和 Z-Index 的 Position 部分，以及带有 Size、Min Size、Max Size、Padding 和 Margin 的 Layout 部分。" width="360"></div>

## 控件

![UI 编辑器底部的控件面板，包含 Full Screen、Container、Image、Label、Button、Input 和 Dropdown 卡片。](/docs/_remote/353fd7589685b5f5d6abd5f2.webp)

该面板将控件分为三类：

* **容器**: **全屏**, **容器**, **图片**
* **文本**: **标签**, **按钮**
* **输入**: **输入**, **下拉框**

**全屏** 是一个容器，会填满放在其中的任何内容，因此非常适合作为需要覆盖屏幕的 UI 的起点。

## 移动与调整大小

工具栏有三种工具模式：

* **自由**：移动和调整大小，通用模式。
* **移动**：仅拖动。
* **调整大小**：仅使用手柄。

位置会吸附到 10 像素网格。按住 **Shift** 拖动时可自由移动，忽略网格。工具按钮旁边的小箭头会打开一个 **吸附** 复选框，可完全关闭吸附。

![2D 工具栏，显示“所有更改已保存”徽标、撤销和重做、Free、Move 和 Resize 工具，以及打开的吸附下拉菜单，其中显示已勾选的 Snap 选项，后面还有播放和停止按钮。](/docs/_remote/5ab6e1170c733fa0011bba2b.webp)

## 布局：流式与自由定位

两个独立属性决定节点最终落在哪里。把它们弄清楚能省去很多困惑。

**Flow** 设置在 **容器** 上，决定它如何排列其子项：

* **自由**：子项会停留在你放置它们的位置。
* **行** / **反向行**：子项从左到右，或从右到左排列。
* **列** / **反向列**：子项从上到下，或从下到上堆叠。
* **换行子项**：当子项放不下时，可让它们换到多行显示。

**忽略布局流** 设置在 **子项** ，决定这个节点是否退出父级的排列。关闭时，由父级来定位它；开启时，你可以通过 Anchor 和 Position 值自行固定它。

由此会产生两个后果：

* 把控件放入 **自由** 容器后，它会停留在你放下它的确切位置。
* 把它放进带有方向的容器中，它就会加入流式布局，排到队列中的位置。

将容器切换为 **自由** 会把现有子项固定在它们当前所在的位置，因此不会跳动。

一个 GUI 的根节点始终采用自由定位。

## 其他值得了解的属性

* **不透明度**：100% 表示完全不透明，0% 表示完全透明。默认值为 100%。
* **场景内嵌区**：GUI 所处的屏幕部分。

  * **全屏** 使用整个可渲染屏幕。
  * **可交互安全区域** 会避开客户端自身的界面，如聊天、小地图和 HUD 指示器。
  * **设备安全区域** 也会避开刘海或系统栏等物理遮挡。画布处于移动预览时会提供此选项。

  在编辑器写出的代码中，这三种选择会变成 `screenInset: 'none'`, `'interactable'` 和 `'device'`。请参见 [屏幕内嵌区域](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area).

如果你为 GUI 的根节点指定固定像素宽高，画布会把它框定为恰好该大小的画板，而不是屏幕。

## 在手机上检查你的 UI

画布右下角的控件可在桌面屏幕和手机之间切换预览。在移动预览中，画布会绘制在手机框中，并带有刘海，这样你就能看到屏幕边缘实际所在的位置。

两组辅助线可帮助你把内容放在玩家看得到、够得着的位置：

* **安全区域辅助线** 会勾勒出你的 GUI 所限于的屏幕部分，与所选的 **场景内嵌区** 一致。当 Scene Inset 为 **设备安全区域** 或 **可交互安全区域** 时，轮廓始终开启，因为 GUI 就是裁剪到这里的。将 Scene Inset 设为 **全屏** 时，你可以通过框架图标按钮打开或关闭轮廓。
* **HUD 辅助线** 会绘制移动客户端自身的控件，如摇杆、跳跃和操作按钮、表情轮盘、个人资料、聊天和指南针。它们仅供参考，你不能选中或移动它们。游戏手柄按钮可将它们打开或关闭，它只会在移动预览中出现，并且当 Scene Inset 为安全区域时，这些辅助线默认显示。

放在安全区域外的内容不会被隐藏，而是会绘制到轮廓之外。这个溢出就是警告：在真实客户端中，它会位于游戏自身的 HUD 下方。

**可交互安全区域** 会清除移动客户端左侧一列控件，但它的右边缘与设备区域共用，因此右下角的操作按钮会按设计覆盖在其上。请保持那些角落为空。参见 [客户端控件所在位置](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/#where-the-client-controls-live).


> [!WARNING]
> **📔 注意**：这些辅助线仅供编辑器近似参考。真实区域由客户端在运行时报告，并且因设备而异，因此一定要在真机上确认。参见 [在移动设备上预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/).


## 自定义移动控件

在移动客户端中，玩家通过一组原生屏幕控件进行移动和操作：摇杆、准星和一排操作按钮。 **MobileHUD** 条目让你无需编写代码就能重塑该 HUD，并在过程中将结果显示在画布上。

一旦你的场景至少有一个 GUI， **MobileHUD** 就会作为 **GUIs** 列表的第一行出现，位于你自己的 GUI 之上。选择它即可在右侧打开其面板。

顶部的三个复选框可隐藏 HUD 的整部分内容：

* **隐藏虚拟摇杆**：移除移动摇杆。
* **隐藏准星**：移除瞄准准星。
* **隐藏输入操作**：一次性移除所有操作按钮。

在它们下方， **Input Actions** 部分会按屏幕上槽位的填充顺序列出这八个按钮：

| 动作                                      | 屏幕上的按钮        |
| --------------------------------------- | ------------- |
| `IA_JUMP`                               | 默认情况下位于中央的大按钮 |
| `IA_POINTER`                            | 交互按钮          |
| `IA_PRIMARY (E)`                        | E 按钮          |
| `IA_SECONDARY (F)`                      | F 按钮          |
| `IA_ACTION_3 (1)` 设置为 `IA_ACTION_6 (4)` | 带数字的按钮        |

每一行提供三项内容：

* **主按钮**：一个单选按钮，可将此动作设为中央的大按钮。 `IA_JUMP` 默认是主按钮，同一时间只能有一个动作成为主按钮。
* 一个 **眼睛图标** 用于显示或隐藏该按钮。隐藏其中一个时，下面的按钮会向上移动以填补空位。
* **自定义图标**：将按钮的图形替换为一个 `.png`, `.jpg` 或 `.jpeg` 来自你场景中的图片。

在此面板中，每个按钮初始时都可见，并且 `IA_JUMP` 是主按钮。


> [!WARNING]
> **📔 注意**：一个从不触碰 HUD 的场景只显示三个按钮：jump、pointer 和 primary。只有在你做出更改后，面板才会写入 `src/mobile-hud.ts` ，因此未改动的面板会保留该默认值。一旦文件存在，它的代码就会用你在这里配置的内容替换默认设置，所以例如只隐藏摇杆，也会把另外五个额外按钮放到玩家屏幕上。请隐藏那些你的场景不使用的按钮。参见 [默认显示内容](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/#what-shows-by-default).



> [!NOTE]
> **💡 提示**：当可见按钮超过五个时，最后几个会被放在一个“+”溢出切换按钮后面。隐藏足够多的上方按钮后，其余按钮会直接显示。参见 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/#how-the-button-layout-works) 了解槽位是如何填充的。


### 它写入的文件

你的更改会写入到 `src/mobile-hud.ts`，它导出一个 `setupMobileHud()` 函数，基于 SDK 的 [`TouchScreenControls`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/) 组件：

```ts
import { engine, InputAction, TouchScreenControls } from '@dcl/sdk/ecs'

export function setupMobileHud() {
  TouchScreenControls.createOrReplace(engine.RootEntity, {
    hideJoystick: true,
    hideCrosshair: false,
    mainAction: InputAction.IA_PRIMARY,
    touchInputs: [
      { inputAction: InputAction.IA_ACTION_3, hide: true },
    ],
  })
}
```

编辑器还会把 import 和调用添加到你的 `src/index.ts`:

```ts
import { setupMobileHud } from './mobile-hud'

setupMobileHud()
```

只有在它有内容可说时，这个文件才会存在。如果你把所有内容都改回默认值，或删除最后一个 GUI，Creator Hub 就会删除 `src/mobile-hud.ts` 并再次移除那两行代码。


> [!WARNING]
> **📔 注意**：当 **MobileHUD** 被选中时，控件面板会被禁用，预览控件也会隐藏，因为此时没有可放置的内容，而且 HUD 仅限移动端。选择你自己的某个 GUI 即可让它们恢复。


## 文件存放位置

UI 编辑器会直接编辑场景的真实源文件。没有单独的保存格式。

* 每个 GUI 都是 `.tsx` 位于 `src/ui/`.
* `src/ui/index.tsx` 是编辑器生成的，用来把它们聚合在一起。不要手动编辑它。
* `src/mobile-hud.ts` 保存你的 [移动控件](#customize-the-mobile-controls)（如果你做过更改）。它是刻意放在 `src/ui/` 之外的，因此绝不会被当作 GUI。

如果你的场景中有一个较旧的单一 `src/ui.tsx` 文件，编辑器会在你第一次打开 UI 编辑器时把它备份为 `src/ui.tsx.bak` 。

因为这些都是普通文件，所以你可以随时在 [代码编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) 中打开它们。有关底层代码的作用，请参阅 [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md) 。

## 编辑器无法编辑的代码

编辑器理解特定形状的 UI 代码。以其他方式编写的节点，例如在循环或条件语句中构建，或使用自定义组件或展开属性写成的节点，会在画布上显示为一个灰色块，并带有一个 **非标准，请在代码中编辑** 徽标。其子节点不会显示，你需要改为在代码中编辑它。

它周围其余的 GUI 仍会正常工作。

## 相关页面

* [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md)：在代码中编写场景 UI。
* [与代码结合](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)：在代码编辑器中打开你的场景。
* [实体与组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/)：编辑器的 3D 侧。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
