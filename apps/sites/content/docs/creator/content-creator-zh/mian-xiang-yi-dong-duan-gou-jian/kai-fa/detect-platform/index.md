> 检测你的场景正在移动端、桌面端还是网页端运行。

# 检测平台

使用 `isMobile()` 用于专门为移动端调整你的 UI、控制和玩法的函数。这是在不完全分叉场景逻辑的情况下，为所有客户端提供出色体验的推荐方式。

## 可用函数

```ts
import { getPlatform, isMobile, isDesktop, isWeb } from '@dcl/sdk/platform'
```

* **`getPlatform()`** — 返回当前平台，类型为 `'mobile' | 'desktop' | 'web' | null`。返回 `null` 直到浏览器已将其平台回报给场景（这会在场景启动后不久发生）。
* **`isMobile()`** — 返回 `true` 如果玩家使用的是移动端客户端。
* **`isDesktop()`** — 返回 `true` 如果玩家使用的是桌面端客户端。
* **`isWeb()`** — 返回 `true` 如果玩家使用的是网页端客户端。


> [!WARNING]
> **📔 注意**：这些函数读取的是在场景启动时异步填充的值，并不能保证在 `main()` 运行时。如果你过早调用它们，它们仍可能返回 `null` / `false`。为稳妥起见，将任何依赖平台的初始化延后，直到 `getPlatform()` 返回的结果不是 `null`，例如在系统中检查：
> 
> ```ts
> import { engine } from '@dcl/sdk/ecs'
> import { getPlatform } from '@dcl/sdk/platform'
> 
> function platformCheckSystem() {
>   if (getPlatform() === null) return
>   engine.removeSystem(platformCheckSystem)
>   setupUI()
> }
> 
> engine.addSystem(platformCheckSystem)
> ```
> 


## 按平台分支你的 UI

一个常见模式是为移动端和桌面端玩家设置不同的 UI：

```ts
import { isMobile, isDesktop } from '@dcl/sdk/platform'

function setupUI() {
  if (isMobile()) {
    // 更大的按钮，更适合触控的简洁布局
    createMobileUI()
  } else if (isDesktop()) {
    // 更紧凑、针对键盘和鼠标优化的布局
    createDesktopUI()
  }
}
```

你也可以用同样的模式来：

* 显示或隐藏针对每种输入方式定制的屏幕提示。
* 在移动端将小的可点击元素替换为更大的触控目标。
* 禁用在移动端不易使用的输入绑定（参见 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/)).

## 检查原始平台值

如果你需要在一个 switch 中处理多个平台，请使用 `getPlatform()`:

```ts
import { getPlatform } from '@dcl/sdk/platform'

const platform = getPlatform()

switch (platform) {
  case 'mobile':
    // 移动端特定行为
    break
  case 'desktop':
    // 桌面端特定行为
    break
  case 'web':
    // 网页端特定行为
    break
  case null:
    // 平台尚未确定
    break
}
```

## 相关

* [移动设备安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/)
* [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/)
* [屏幕 UI](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
