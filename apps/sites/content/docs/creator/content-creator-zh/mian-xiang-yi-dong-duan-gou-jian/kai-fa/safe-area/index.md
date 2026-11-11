> 场景 UI 在移动端可以安全显示的区域——避开设备硬件边距和客户端自身控制。

# 移动端安全区域

在手机上，有两种互不相关的东西会侵占你的 UI 能安全使用的屏幕空间：

* **设备的硬件边距** ——刘海或摄像头开孔、状态栏、Home 指示条以及圆角。绘制在这些区域下面的 UI 会被部分遮挡，或很难点击。
* **Decentraland 客户端自身的控制项** ——摇杆、聊天、个人资料、相机控制、动作按钮。放在这些控件下方的场景 UI 在视觉上会冲突，并会争夺同样的点击。

你无需测量这两者中的任何一个。客户端会在运行时报告这两个区域，SDK 会将你的 UI 放置在你选择的区域内，并通过 [`screenInset`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area) 渲染器选项：

| 你想要的                | 要传入的值                                 |
| ------------------- | ------------------------------------- |
| 避开硬件边距              | 不传入任何值 — `screenInset: 'device'` 是默认值 |
| 避开硬件边距 **和** 客户端控制项 | `screenInset: 'interactable'`         |
| 整个屏幕，边距由你自行处理       | `screenInset: 'none'`                 |

```ts
// 建议用于不能与移动端控制项冲突的场景 UI
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```


> [!WARNING]
> **客户端支持**: `'interactable'` 需要移动端客户端 `1.12.1` 或更新版本。在旧版客户端中，该区域会报告为 0，UI 会退回到覆盖整个屏幕。同一版本还统一了 `'device'` 在 Android 和 iOS 之间的区域。
> 
> **在桌面端这并不是无操作** ——桌面客户端会为自身 UI 预留屏幕左侧约 25% 的区域，因此 `'interactable'` 也会改变你的桌面布局。若只想在手机上使用该内边距，请分支处理 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) 。


参见 [屏幕内边距区域](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area) 完整参考：真实设备上并排展示的三个区域、各渲染器的行为，以及它们与 [虚拟屏幕](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#default-virtual-size).

## 客户端控制项所在的位置

`'interactable'` 会给你这个区域 **每个 explorer 为场景 UI 指定的区域** ——这并不意味着所有客户端控制项都在其外部。对于移动端客户端，它是设备安全区域减去左侧栏（聊天、个人资料、摇杆、表情）；右下角的动作按钮会绘制在该区域之上 **这是设计如此**.

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the mobile client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> 在移动端客户端上 <code>1.12.1</code>。洋红色矩形是场景 UI，绿色轮廓表示可交互区域。左侧栏被排除在外；右下角的动作按钮位于该区域之上。</p></figcaption></figure>

因此，即使使用 `'interactable'`，也要将这些位置视为拥挤区域：

* **右下角** ——动作按钮和交互按钮会叠加在上面。放在这里的内容仍然可触达，但会争抢点击。
* **右上角** ——个人资料和相机控制位于该区域外侧，因此贴近那个角落的 UI 会被视为客户端 HUD 的一部分。

该区域覆盖哪些内容由各个 explorer 自行决定，并且可能在不同版本之间变化。请在运行时读取，不要硬编码，并在你要支持的平台上进行检查。

## 场景 UI 放在哪里

* **屏幕中央** ——需要玩家阅读并作出回应的可操作对话框。
* **顶部居中** ——无需操作的消息、状态和通知。
* **底部居中，位于交互按钮上方** ——上下文相关提示。
* **不要放在右下角** ——即使在可交互区域内，这里也属于动作按钮的范围。

## 改为包裹你 UI 的一部分

如果你通过 `screenInset: 'none'` 并且只想保护 UI 的一部分，请将其包裹在 `ScreenInsetArea` 或 `InteractableArea` 组件中。每个组件都会读取其对应的区域，并将其应用到子元素。请从 `@dcl/sdk/react-ecs`:

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity, ScreenInsetArea } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUi() {
  ReactEcsRenderer.setUiRenderer(() => (
    <ScreenInsetArea>
      {/* 一个大小为 100% × 100% 的子元素会正好填满安全内边距区域 */}
      <UiEntity
        uiTransform={{ width: '100%', height: '100%' }}
        uiBackground={{ color: Color4.create(0, 0, 0, 0.5) }}
      />
    </ScreenInsetArea>
  ), { screenInset: 'none' })
}
```

这两个组件都会使用客户端报告的值进行绝对定位，因此 `positionType` 和 `位置` 字段被保留——你为它们设置的任何值都会被忽略。所有其他 `uiTransform` 字段被保留——任何你为它们设置的值都会被忽略。所有其他 `uiTransform` 属性（`padding`, `flexDirection`, `alignItems`，…）以及 UI 组件（`uiBackground`, `onMouseDown`，…）会照常工作。当区域发生变化时，例如旋转或系统栏显示/隐藏时，它们会自动响应，并会补偿 [UI 缩放因子](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-virtual-scale)，因此无论你的虚拟屏幕尺寸如何，边距都会正确落位。


> [!WARNING]
> **📔 不要重复应用内边距：** 这段代码中的 `screenInset: 'none'` 很重要。若在渲染器已经应用了对应内边距的同时，又将 UI 包裹在该组件中，就会使你的 UI 向内偏移双倍边距。二者择一即可。



> [!NOTE]
> **📱 仅限移动端：** 设备内边距只有在 **移动端客户端**上才有真实值。在 **桌面客户端** 它们是 `(0, 0, 0, 0)`，因此 `screenInset: 'device'` 和 `ScreenInsetArea` 组件在那边不会产生任何效果，你的 UI 渲染结果与未使用它们时完全相同。与设备内边距不同，可交互区域 *是* 在桌面端并非 0。


## 为何这很重要

与客户端控制项重叠的场景 UI 会：

* 部分被摇杆、交互按钮或相机控制遮挡。
* 与这些控制项争抢点击——玩家可能会不小心触发其中一个。
* 让你的场景在移动端显得有问题，从而影响推荐展示和留存。

务必使用 [预览二维码](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/).

## 相关

* [屏幕内边距区域](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area) ——完整的 `screenInset` 参考。
* [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/)
* [从代码中检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) ——使用 `isMobile()` 来切换布局。
* [屏幕 UI](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/)
* [UX 与 UI 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
