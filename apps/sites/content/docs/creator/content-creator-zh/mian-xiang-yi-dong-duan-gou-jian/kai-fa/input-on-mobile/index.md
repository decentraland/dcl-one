> 在 Decentraland 移动客户端中，输入动作如何映射为触控。

# 移动端输入

Decentraland 的输入系统旨在与设备无关。相同的 `InputAction` 在桌面端由 SDK 暴露的枚举在移动端也会从屏幕上的控件路由过来，因此大多数场景无需任何更改即可运行。不过，在为触控构建时，有一些规则和注意事项值得了解。

有关完整的输入模型，请参见 [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)。本页重点介绍移动端特有的内容。

<figure><img src="/docs/_remote/d29f6b96a1d1db9049d689d6.webp" alt="The on-screen controls with the + menu expanded, showing every input button"><figcaption><p>“+”菜单展开时的屏幕控件——所有可见的输入操作</p></figcaption></figure>

## 触控映射到什么

每一个 `InputAction` 在桌面端存在的，也都可在移动客户端中使用——屏幕控件只是将触摸输入路由到相同的 SDK 输入。这意味着任何依赖标准 `InputAction` 值的场景都能在移动端开箱即用。

屏幕上的控件映射如下：

* **屏幕虚拟摇杆** ——控制 `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` 以及角色移动。
* **交互按钮（右下角）** ——触发 `IA_POINTER` （桌面端的鼠标左键）作用于玩家瞄准的目标。
* **E 按钮** ——触发 `IA_PRIMARY` （即 `E` 键在桌面端）。
* **F 按钮** ——触发 `IA_SECONDARY` （即 `F` 键在桌面端）。默认隐藏（见下文）。
* **跳跃按钮** ——触发 `IA_JUMP` （即 `空格` 键在桌面端）。这是 HUD 上最大的按钮，并且始终容易触及。
* **1 / 2 / 3 / 4 按钮** ——分别触发 `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` 。默认隐藏（见下文）。
* **拖动摄像机** ——旋转摄像机；不会作为 `InputAction`.

## 控件自定义

场景可以重新配置屏幕上的控件——隐藏摇杆或准星，隐藏任意按钮（包括跳跃按钮），更改中央按钮的功能，替换按钮图标，或者完全用自定义 UI 替换原生控件。请参见 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/) 和 [UI 输入绑定](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding/).

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="Customized on-screen controls"><figcaption><p>自定义 HUD：一个重新设图标、映射到不同操作的中央按钮</p></figcaption></figure>

## 移动端关键操作应避免使用的输入

每一个 `InputAction` 可以路由到屏幕按钮，但使用 SDK 7.32.0 或更高版本构建的场景默认只显示其中三个：交互按钮、E 按钮和跳跃按钮。 `IA_SECONDARY` （F）和 `IA_ACTION_3`–`IA_ACTION_6` （即 `1`/`2`/`3`/`4` 按钮） **默认隐藏**，因此绑定到其中之一的按键操作在手机上不会有按钮可用，除非你的场景将其显示出来。在较旧的 SDK 版本中，所有八个按钮都会显示，其中优先级较低的按钮会被放在一个“+”溢出菜单后面，在游戏过程中不太容易触及。

除非你的场景明确使用 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/)显示它们，否则避免将按键操作绑定到：

* `IA_SECONDARY` —— `F` 键在桌面端 / `F` 按钮在移动端
* `IA_ACTION_3` —— `1` 键在桌面端 / `1` 按钮在移动端
* `IA_ACTION_4` —— `2` 键在桌面端 / `2` 按钮在移动端
* `IA_ACTION_5` —— `3` 键在桌面端 / `3` 按钮在移动端
* `IA_ACTION_6` —— `4` 键在桌面端 / `4` 按钮在移动端

建议改为：

* `IA_POINTER` 用于主要点击 / 交互按钮
* `IA_PRIMARY` 用于 E 按钮动作
* `IA_JUMP` 当某个动作自然对应跳跃时使用（它是移动端 HUD 上最大、最容易触及的按钮）
* 基于接近的触发器（见 [接近事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/)）当某个动作应在玩家接近实体时自动触发时

## 光标锁定

该 [`PointerLock` 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#lock-or-unlock-the-cursor) 是桌面客户端的概念（锁定与未锁定的鼠标光标）。它不适用于移动端的触摸，保留在场景中是安全的——在那里它不会产生任何影响。

## 相关内容

* [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/)
* [UI 输入绑定](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding/)
* [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)
* [接近事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/)
* [从代码中检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/)
* [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
