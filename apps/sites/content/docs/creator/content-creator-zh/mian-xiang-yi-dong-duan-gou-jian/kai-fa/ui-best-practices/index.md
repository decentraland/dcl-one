> 设计在移动端表现良好的场景 UI 的注意事项与禁忌。

# UI 最佳实践

目前还没有一套经过验证、适用于 Decentraland 移动端 UI 的通用方案——平台尚新，我们仍在不断迭代。本页建议是从真实场景实践中提炼出的当前最佳做法。请将其作为起点，并在真实设备上进行测试。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/5OmTTzpPdDc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=5OmTTzpPdDc>">Watch on YouTube</a></p>



## 推荐做法

* **设计移动端专用 UI**，或根据屏幕尺寸和平台调整 UI。使用 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) 进行分支处理。
* **将关键 UI 保持在** [**安全区域**](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/)**.** 默认情况下，设备的硬件边距（刘海、状态栏、主屏幕指示条）会为你自动留出——只需不要通过以下方式退出此行为： `screenInset: 'none'`。客户端自身的控件属于另一个区域，而 `screenInset: 'interactable'` 涵盖了其中一部分工作：它会将你的 UI 定位在浏览器为场景 UI 指定的矩形区域内。在移动端，这会避开左侧控件；右下方的操作按钮按设计会覆盖该区域，因此请留意 [这些角落](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/#where-the-client-controls-live) 。 `'interactable'` 需要移动端客户端 `1.12.1` 或更高版本——旧版客户端完全不会报告边距，UI 会覆盖整个屏幕。
* **尽量减少选项。** 只显示玩家当前所需的内容，其余内容逐步展示。
* **将可操作对话框置于屏幕中央** ——凡是玩家需要阅读并作出响应的地方。
* **将不可操作的信息置于顶部中央** ——状态、通知和环境信息。
* **在全屏 UI 后隐藏触控控件。** 当记分板、商店或结果面板打开时，玩家不需要跳跃或按 E 键。参见 [操作按钮上的全屏 UI](#full-screen-ui-over-the-action-buttons).

## 不推荐做法

* **不要在未考虑虚拟屏幕的情况下，纯粹按像素设置 UI 元素尺寸。** 像素值会根据以下内容缩放： `virtualWidth` / `virtualHeight` 中描述的参考分辨率 [屏幕 UI](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) —— `1600x720` 在移动端默认情况下， `1920x1080` 在桌面端和 Web 端。请了解你的像素值所依据的参考分辨率；若并非默认值，请显式传入，并结合平台感知的尺寸调整。仅当你确实需要原始画布像素时，才禁用虚拟屏幕（传入尺寸 `0`）。
* **不要将元素放在安全区域之外。** 它们会与客户端控件发生冲突。
* **不要将 UI 锚定在左上角或左边缘。** 客户端会在该处绘制聊天、个人资料和虚拟摇杆，且都会覆盖在你的场景 UI 上。请改为锚定在右侧或中央，或者使用 `screenInset: 'interactable'`。请参见 [永远不要将 UI 锚定到左上角](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).
* **不要将交互元素放在右下角。** 操作按钮会覆盖该区域，点击该处会触发客户端按钮，而不是你的场景。
* **不要依赖小按钮。** 在触摸屏上，小目标不易准确点击。
* **不要将按键操作绑定到 `IA_SECONDARY` 或 `IA_ACTION_3`–`IA_ACTION_6`** （即 `F` 和 `1`/`2`/`3`/`4` 键盘上的按键），除非你的场景显示这些按钮。它们在移动端默认隐藏（SDK 7.32.0+）。参见 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/).

## 操作按钮上的全屏 UI

`screenInset: 'interactable'` 可让你的 UI 避开左侧控件，但右下角的操作按钮按设计会覆盖该区域。即使面板填满该区域，仍会在该处被覆盖，并且在该区域的点击会触发客户端按钮而不是你的 UI。

对于必须在 **游戏 HUD** 中保持可用的 UI，当玩家移动时，请将其避开右下角：将其锚定在右上角或顶部中央。

对于必须在 **会中断游戏玩法的 UI**，例如记分板、商店或结果界面，在其打开时隐藏触控控件，并在关闭时恢复：

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

function openScoreboard() {
  TouchScreenControls.hideAll()
  TouchScreenControls.hideJoystick()
  // ……显示你的 UI
}

function closeScoreboard() {
  TouchScreenControls.showDefault()
  TouchScreenControls.showJoystick()
  // ……隐藏你的 UI
}
```

使用 `showDefault()` 而不是 `showAll()` 以恢复按钮：场景开始时仅显示跳跃、指针和主操作按钮，因此 `showAll()` 会恢复玩家原本没有的五个按钮。三按钮默认设置和 `showDefault()` 都需要 SDK 7.32.0 或更高版本；在较旧的 SDK 上，全部八个按钮都会显示，而 `showAll()` 是恢复它们的唯一方式。 `showDefault()` 只会恢复游戏手柄按钮，因此请与 `showJoystick()`搭配使用。这两个辅助工具在桌面端均不起作用，因此无需根据平台进行分支处理。参见 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/) 以查看完整组件。

如果面板打开时还应让头像停止移动，请额外添加一个 [输入修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/#freeze-the-player) 。

## 尺寸调整

旧有的经验法则—— *先在桌面端设计，再将 UI 尺寸放大 3 倍用于移动端* ——早于 SDK 的两项变更，不应再盲目套用。其中大部分 3 倍缩放现在已自动为你完成：

* **`devicePixelRatio` 不参与 UI 布局。** 以像素为单位的 UI 现在会渲染得 `devicePixelRatio` 倍于过去——通常为 **在手机上为 2–3 倍**，在视网膜屏笔记本电脑上也是如此。无法退出此行为。
* **虚拟屏幕会因平台而采用不同默认值**, `1600x720` 在移动端基于 `1920x1080` 在桌面端和 Web 端，因此同一个像素值在手机上已覆盖约多 1.2 倍的屏幕区域。

先保持桌面端尺寸不变，从真实设备上测量——参见 [在移动设备上预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) ——仅在点击目标或文本确实不足时再放大。使用 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) 在两者确实需要不同时进行分支处理。

## 相关内容

* [缺失功能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/missing-features/)
* [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/)
* [从代码中检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/)
* [屏幕 UI](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/)
* [UX 与 UI 指南](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
