> 将输入动作绑定到你自己的 UI 元素上，让它们驱动玩家输入。

# UI 输入绑定

`UiInputBinding` 将任何 UI 元素变成一个控制按钮。将一个元素绑定到一个或多个 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons)s，并且在按住它时——通过触摸或指针——这些动作会一直保持按下，驱动本地玩家的输入（移动、跳跃）以及任何场景中的 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 监听器，和原生屏幕按钮完全一样。

用它来构建你自己的触控控制。通常会与 [屏幕控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/)：隐藏原生按钮，然后用你自己的替换它们。

<figure><img src="/docs/_remote/19dc4254d88476f6abb9fbcf.webp" alt="A retro-style gamepad assembled from custom UI elements, each wired to an input action"><figcaption><p>一个完全自定义的游戏手柄，由 UI 元素构成——每个按钮都是绑定到某个输入动作的 UI 实体</p></figcaption></figure>

## 将一个动作绑定到一个 UI 元素

添加一个 `uiInputBinding` 属性到你任意一个 `.tsx` UI 中，并列出按下时要保持的动作。该属性可用于每个 UI 元素（[`UiEntity`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#ui-entities), [`Button`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#ui-entities), [`标签`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#ui-entities)，……），就像 `uiTransform` 和 `uiBackground`.

**一个按住时让玩家前进的按钮：**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button } from '@dcl/sdk/react-ecs'

export const forwardButton = () => (
	<Button
		value="▲"
		uiTransform={{ width: 100, height: 100 }}
		uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
	/>
)
```

## 构建一个自定义控制组

将多个已绑定元素组合起来，构建完整的控制方案。

**一个移动按钮加一个触发 `IA_PRIMARY`:**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button, UiEntity } from '@dcl/sdk/react-ecs'

export const customControls = () => (
	<UiEntity uiTransform={{ width: 240, height: 120 }}>
		<Button
			value="▲"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
		/>
		<Button
			value="使用"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_PRIMARY] }}
		/>
	</UiEntity>
)
```

绑定的动作行为与原生按钮完全一样： `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` 移动头像，而任何动作都可以被你的场景的 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 监听器读取。移除该属性（或组件）会解除绑定。

## 属性

| 属性        | 类型                                                                                                                           | 描述                                                                                                                                                                                                                                                          |
| --------- | ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `actions` | 数组 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) | 当此元素被按下时保持按住的输入动作。参见 [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/) 以查看完整的 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 值列表。 |


> [!NOTE]
> 将其与 [`TouchScreenControls`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/) 以隐藏原生控制并用你自己的触控 UI 替换它们——这是发布完全自定义控制方案的推荐方式。


## 相关

* [屏幕控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/)
* [UI 按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_button_events/)
* [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/)
* [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
