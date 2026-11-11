> 为你的场景配置原生的屏幕触控控制。

# 屏幕控制

在移动客户端上，玩家通过一组原生屏幕控制与你的场景交互——一个虚拟摇杆、一个准星和一组手柄按钮。该 `TouchScreenControls` 组件可让你的场景重塑该 HUD：减少杂乱、隐藏摇杆或准星、更改中央大按钮的功能、将某个按钮的图标替换为你自己的图标，或者直接隐藏这些按钮并用你自己的 UI 取而代之。

<figure><img src="/docs/_remote/8414c098957fe0e8df7af46b.webp" alt="The default mobile HUD: virtual joystick on the left, gamepad buttons on the right"><figcaption><p>自定义之前的默认屏幕控制</p></figcaption></figure>


> [!NOTE]
> 该组件会在玩家进入你的场景时自动生效，并在他们离开的一瞬间恢复默认值，因此不使用它的场景不受影响。它只影响带有原生屏幕控制的平台：在桌面端它不会执行任何操作，在 VR 中也没有效果。


## 默认显示内容

来自 SDK `7.32.0`，每个场景开始时都会显示三个手柄按钮：

* `IA_JUMP`，中央大按钮
* `IA_POINTER`，交互按钮
* `IA_PRIMARY`，E 按钮

`IA_SECONDARY` 和 `IA_ACTION_3` 设置为 `IA_ACTION_6` 默认隐藏。你只需要 `TouchScreenControls` 当你想要与此不同的内容时。


> [!WARNING]
> **📔 注意**：三按钮默认值以及 `showDefault()` 辅助函数需要 SDK `7.32.0` 或更高版本。更早的版本中，除非你的场景隐藏了某些按钮，否则八个按钮都会显示，并且 `showDefault()` 不存在，因此 `showAll()` 是将按钮恢复显示的唯一方式。


SDK 会在场景开始时、第一帧之前只写入一次该默认集合；如果你的场景已经选择了自己的按钮，则会跳过它。若满足以下任一条件，你的场景就算已选择：

* 其中一个按钮可见性辅助函数已经运行过： `hideAll()`, `showAll()`, `showDefault()` 或 `hide()`.
* 根实体上的该组件已经至少列出一个按钮，例如因为你的场景用 `createOrReplace` 在……的顶层 `index.ts`，位于……之外 `main()`.

`hideJoystick()`, `showJoystick()`, `hideCrosshair()`, `showCrosshair()` 和 `setMainAction()` 不算：默认按钮集仍会与它们一起生效。

你的 `main()` 函数会在该写入之后运行，因此你在其中所做的任何操作都会叠加在默认集合之上。像 `hide()` 这样的辅助函数会与其合并，而 `createOrReplace` 则会用你的列表替换整个列表。

要把隐藏的按钮带回来，请调用 `showAll()` 对全部八个按钮都调用。 `showDefault()` 会恢复默认集合，并保持你的图标、摇杆、准星和主操作不变。

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// 显示所有屏幕按钮，包括 1 / 2 / 3 / 4 按钮
TouchScreenControls.showAll()
```

## 按钮布局如何工作

手柄按钮构成一个单一的 **优先级栈**。顺序是固定的：

1. `IA_JUMP`
2. `IA_POINTER`
3. `IA_PRIMARY` (E)
4. `IA_SECONDARY` (F)
5. `IA_ACTION_3` (1)
6. `IA_ACTION_4` (2)
7. `IA_ACTION_5` (3)
8. `IA_ACTION_6` (4)

屏幕上的位置也同样固定。那些 **可见** 按钮会从栈顶向下填充这些位置——所以你改变的是 *哪些* 按钮可见，以及 *哪个按钮领先*，而不是它们的顺序。

| 当你……时                     | 控件……                                                                |
| ------------------------- | ------------------------------------------------------------------- |
| **隐藏一个按钮** （任何按钮，包括跳跃）    | 每个优先级更低的按钮都会上移以填补空位。隐藏跳跃按钮后， `IA_POINTER` 会占据中央位置。                  |
| **什么都不改**                 | 默认集合适用： `IA_JUMP` 是中央大按钮，旁边有 `IA_POINTER` 和 `IA_PRIMARY` 位于它周围的槽位中。 |
| **设置主按钮** 使用 `mainAction` | 该动作会跃至最前并成为中央按钮；其他所有按钮保持其正常顺序。                                      |
| **设置一个同时也被隐藏的主按钮**        | 隐藏优先——该按钮仍会保持隐藏。                                                    |
| **保持 5 个或更少按钮可见**         | 它们会全部直接显示（中央按钮加上周围最多四个按钮）；不会有“+”菜单。                                 |
| **保持 5 个以上按钮可见**          | “+”占据最后一个位置，因此会直接显示四个按钮（中央按钮加上三个），其余按钮则位于“+”溢出切换按钮之后。               |

<figure><img src="/docs/_remote/ac9947bc2b985676d54e81a8.webp" alt="Three HUDs showing the gamepad reflowing as the number of visible buttons changes"><figcaption><p>按钮如何随可见数量重新排布。 <strong>左侧（7 个按钮）：</strong> 主按钮、1–3 的弧形排列，以及“+”在一个向上延伸的列中承载溢出内容（4–7）。 <strong>中间（5 个按钮）：</strong> 相同的弧形排列，且“+”后面的溢出列更短（4–5）。 <strong>右侧（4 个按钮）：</strong> 四个都直接显示，“+”消失。</p></figcaption></figure>


> [!NOTE]
> 该 `1`/`2`/`3`/`4` 按钮默认隐藏。 `showAll()` 会把它们带回来，但这会留下八个可见按钮，因此优先级最低的按钮最终会落到“+”后面。要让带编号的按钮直接显示在屏幕上，请将可见数量保持在五个或更少：只显示你的场景实际使用的按钮。


## 常见任务

`TouchScreenControls` 提供了一组便捷辅助函数。每个函数都会将该组件写入场景的 `RootEntity` （客户端从这里读取它）并与当前值合并，所以你可以从任何地方调用它们。

**更改主按钮** ——让中央大按钮触发不同的动作：

```ts
import { TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.setMainAction(InputAction.IA_PRIMARY)
}
```

**隐藏摇杆或准星** ——移除移动摇杆和/或瞄准准星，并用它们的 `显示` 对应函数把它们带回来：

```ts
TouchScreenControls.hideJoystick()
TouchScreenControls.hideCrosshair()

// 要把它们带回来：
TouchScreenControls.showJoystick()
TouchScreenControls.showCrosshair()
```

**隐藏特定按钮** ——传入你想移除的动作（其余会级联上移）：

```ts
TouchScreenControls.hide([InputAction.IA_SECONDARY, InputAction.IA_JUMP])
```

**隐藏所有按钮、显示所有按钮，或者回到默认集合：**

```ts
TouchScreenControls.hideAll()     // 完全没有手柄按钮
TouchScreenControls.showAll()     // 全部八个按钮
TouchScreenControls.showDefault() // 仅跳跃、指针和主按钮
```

这三个只影响手柄按钮。它们不会恢复隐藏的摇杆或准星，也不会更改你的自定义图标或主操作。请使用 `showJoystick()` / `showCrosshair()` 用于这些。

**替换按钮图标** ——若要完全控制（自定义图标、一次进行多项更改），请将原始组件写入 `engine.RootEntity`:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideCrosshair: true,
		mainAction: InputAction.IA_PRIMARY,
		touchInputs: [
			{
				inputAction: InputAction.IA_PRIMARY,
				icon: { tex: { $case: 'texture', texture: { src: 'images/grab.png' } } },
			},
		],
	})
}
```

辅助函数一览：

| 辅助函数                    | 功能                                                     |
| ----------------------- | ------------------------------------------------------ |
| `setMainAction(action)` | 设置中央大按钮触发的动作。                                          |
| `hideJoystick()`        | 隐藏原生虚拟摇杆。                                              |
| `showJoystick()`        | 再次显示原生虚拟摇杆。                                            |
| `hideCrosshair()`       | 隐藏屏幕准星/瞄准圈。                                            |
| `showCrosshair()`       | 再次显示屏幕准星/瞄准圈。                                          |
| `hide(actions)`         | 隐藏给定的手柄按钮（与当前配置合并）。                                    |
| `hideAll()`             | 隐藏所有手柄按钮。                                              |
| `showAll()`             | 显示全部八个手柄按钮（清除按钮隐藏列表）。这会 **未** 影响摇杆/准星。                 |
| `showDefault()`         | 恢复默认集合：跳跃、指针和主按钮可见，次要按钮以及动作 3 到 6 隐藏。保持图标、摇杆、准星和主操作不变。 |

## 属性

在你使用 `createOrReplace`:

| 属性              | 类型                                                                                                                        | 说明                                                                                                                     |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `hideJoystick`  | *布尔值*                                                                                                                     | 隐藏原生虚拟移动摇杆。                                                                                                            |
| `hideCrosshair` | *布尔值*                                                                                                                     | 隐藏屏幕准星/瞄准圈。                                                                                                            |
| `mainAction`    | [*InputAction*](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) | 将此动作移到栈前，使其成为中央大按钮；其他按钮保持其顺序。只有手柄动作有效（见下文）。未设置时，首个可见按钮（`IA_JUMP` 默认情况下）领先。参见 [按钮布局如何工作](#how-the-button-layout-works). |
| `touchInputs`   | *数组*                                                                                                                      | 每个按钮的覆盖设置。写入此字段会替换整个列表，因此你省略的任何按钮都会以其默认图标显示，包括最初隐藏的按钮。                                                                 |

每个 `touchInputs` 条目包含：

| 字段            | 类型                                                                                                                        | 说明                                                                                                                                                                                                            |
| ------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inputAction` | [*InputAction*](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) | 此条目配置哪个屏幕按钮。                                                                                                                                                                                                  |
| `hide`        | *布尔值*                                                                                                                     | 隐藏此按钮。默认值为 `false` （显示）。任何按钮都可以隐藏， **包括 `IA_JUMP`** ——其余按钮会级联上移以填补其位置。                                                                                                                                        |
| `icon`        | [*TextureUnion*](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_background/#background) （可选）                      | 用场景图片覆盖该按钮图标。使用 `texture` 带有内容映射的 `src` （即包含在你场景中的图像）—— `{ tex: { $case: 'texture', texture: { src: 'images/grab.png' } } }`。仅支持场景内容路径（不支持外部 URL、头像或视频纹理）。对于跳跃按钮，这会替换其所有动态状态（跳跃 / 二段跳 / 滑翔）。如果路径无法解析，则使用内置图标。 |

## 哪些动作映射到哪些按钮

该 [`InputAction`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 这里的值与各处使用的值相同 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/) 和 [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)。以下是映射到屏幕按钮的动作：

| InputAction                                                   | 屏幕按钮             |
| ------------------------------------------------------------- | ---------------- |
| `IA_JUMP`                                                     | 中央大按钮（默认）        |
| `IA_POINTER`                                                  | 交互按钮             |
| `IA_PRIMARY`                                                  | E 按钮             |
| `IA_SECONDARY`                                                | F 按钮             |
| `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` | 1 / 2 / 3 / 4 按钮 |


> [!WARNING]
> `IA_ANY` 和 `IA_MODIFIER` 是元值——它们不会映射到按钮，也不能在这里使用。


## 示例

要隐藏摇杆，请保持 F 按钮和编号按钮隐藏，并为中央跳跃按钮使用随你的场景一起打包的自定义图标：

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideJoystick: true,
		touchInputs: [
			// 这个列表会替换默认列表，因此通常最初
			// 隐藏的按钮也必须在这里列出
			{ inputAction: InputAction.IA_SECONDARY, hide: true },
			{ inputAction: InputAction.IA_ACTION_3, hide: true },
			{ inputAction: InputAction.IA_ACTION_4, hide: true },
			{ inputAction: InputAction.IA_ACTION_5, hide: true },
			{ inputAction: InputAction.IA_ACTION_6, hide: true },
			{
				inputAction: InputAction.IA_JUMP,
				icon: { tex: { $case: 'texture', texture: { src: 'images/banana.png' } } },
			},
		],
	})
}
```

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="A mobile HUD with the joystick hidden and the central jump button showing a custom banana icon"><figcaption><p>结果：摇杆消失，编号按钮隐藏（因此“+”消失），跳跃按钮图标已更换</p></figcaption></figure>

要完全替换原生控件，请在这里将它们隐藏，并使用 [UI 输入绑定](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding/).

## 相关内容

* [UI 输入绑定](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_input_binding/)
* [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/)
* [点击事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/)
* [从代码中检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
