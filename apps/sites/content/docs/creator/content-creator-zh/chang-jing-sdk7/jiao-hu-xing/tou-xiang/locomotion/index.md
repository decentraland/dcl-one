> 更改玩家的速度和跳跃高度，或限制他们的移动方式

# 移动设置与限制

你可以更改玩家的头像在你的场景内移动的方式。使用 `AvatarLocomotionSettings` 组件来调整速度和跳跃高度，以及 `InputModifier` 组件来冻结玩家或阻止特定类型的移动。这两个组件都添加在 `engine.PlayerEntity`上，并且它们都只会在玩家位于你的场景边界内时生效。

## 移动设置

你可以影响玩家的移动能力，例如奔跑速度、跳跃高度等。这些都可以动态调整，例如允许玩家通过与某个物品交互获得临时的速度提升，或在短时间内禁用玩家的跳跃能力。


> [!NOTE]
> **💡 提示**：你无需编写代码即可设置这些值。将 **移动设置** 智能物品拖入你的场景中，在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/) 并调整其滑块。参见 [移动设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/#locomotion-settings).


要做到这一点，请添加一个 `AvatarLocomotionSettings` 组件到场景中的 `engine.PlayerEntity`.

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	jumpHeight: 2,
})
```

可用的属性如下：

* `walkSpeed`：玩家步行的速度，以米/秒计。在桌面客户端上，玩家按下 Control 键即可步行。
* `jogSpeed`：玩家慢跑的速度，以米/秒计。这是玩家默认的移动方式。
* `runSpeed`：玩家奔跑的速度，以米/秒计。在桌面客户端上，玩家按下 Shift 键即可奔跑。
* `jumpHeight`：玩家跳跃的高度，以米计。
* `runJumpHeight`：玩家在奔跑后跳跃的高度，以米计。
* `doubleJumpHeight`：双重跳跃时第二次跳跃的高度，以米计。
* `glidingSpeed`：玩家在滑翔时移动的水平速度，以米/秒计。
* `glidingFallingSpeed`：玩家在滑翔时的最大下落速度，以米/秒计。这只会限制玩家的下坠：向上的运动，例如场景持续力带来的上升，不会受限。
* `hardLandingCooldown`：重落地后的冷却时间，以秒计。这是玩家从高处坠落着地后，在能够再次移动前需要等待的时间。

供参考，这些属性的默认值如下：

* `walkSpeed`：1.5 米/秒
* `jogSpeed`：8 米/秒
* `runSpeed`：10 米/秒
* `glidingSpeed`：6 米/秒
* `glidingFallingSpeed`：1 米/秒
* `jumpHeight`：1 米
* `runJumpHeight`：1.5 米
* `doubleJumpHeight`：2 米
* `hardLandingCooldown`：0.75 秒


> [!NOTE]
> **💡 提示**：在滑翔时，场景施加的持续力会增强 1.5 倍，向上的力可以将玩家抬起。参见 [滑翔时的力](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics/#forces-while-gliding).



> [!NOTE]
> **💡 提示**：这些属性都不能小于 0。如果你将其中任何一个设为负值，它将被钳制为 0。将这些值设为 0 的效果与使用 `InputModifier` 来阻止某些按键的使用相同。
> 
> 只有当玩家位于场景边界内时，你才能影响其移动能力。要影响其他玩家的头像，你必须在他们各自的实例上运行影响其移动能力的代码。


你可以创建一个 [智能穿戴物](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/) ，使玩家始终跑得更快或跳得更高。如果场景和智能穿戴设备都为这些参数定义了不同的值，则始终使用场景中的值。

为确保在跑酷场景中没有人获得不公平的优势，你可以通过在场景中显式添加这些默认值来强制使用默认参数：

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	：1.5,
	jogSpeed: 8 ,
	jumpHeight: 1,
	runJumpHeight: 1.5,
	hardLandingCooldown: 0.75
})
```

## 限制移动

你可以限制玩家在你的场景中可以执行的操作。可用它来冻结玩家，或限制特定形式的移动，例如阻止玩家跳跃或奔跑。

### 冻结玩家

你可以冻结玩家，使任何输入键都无法移动头像。这对许多游戏机制都很有用。在播放不应被移动打断的重要动画时，或在 [Virtual Camera](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/) 角色背对头像而你不希望玩家盲目移动时，冻结玩家也是一种良好的做法。

使用 `InputModifier` 组件，放在 `engine.PlayerEntity` 以防止玩家的输入影响头像的移动。头像将保持静止，玩家只能旋转摄像机。

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: true,
	}),
})
```

请记住以下注意事项：

* 在禁用玩家交互时，他们的头像仍会受到外部力量的影响，例如重力或移动平台。
* 该 `InputModifier` 组件只能与 `engine.PlayerEntity` 实体一起使用。它只能影响当前玩家，不能影响其他玩家。
* 此组件仅在头像位于你的场景边界内时影响玩家。一旦他们离开场景，其移动限制就会停止。
* 在禁用玩家交互时，玩家不能自由做表情动作，但场景仍可触发头像上的动画。
* 玩家输入不会影响头像，但 [全局输入事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/#global-input-events) 仍可被场景监听。你可以用它们来控制载具，或者使用 [Virtual Camera](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/) 跟随另一个实体移动，将其视为替代头像。

### 限制特定类型的移动

你可以不完全冻结玩家，而是限制玩家的某些特定移动方式。这可用于游戏玩法方面的原因，例如通过禁止二段跳和滑翔来保持平台跳跃游戏的难度。这些能力甚至可以作为游戏机制动态切换，例如给玩家一个耐力条，并在其耗尽时禁止他们奔跑。它也可用于营造场景氛围，例如在一个希望显得宁静的地点禁止奔跑或跳跃。 `InputModifier` 包含以下选项：

* `disableWalk`：玩家不能慢走（按下 Control 键）。如果玩家尝试步行，在允许的情况下，他们会改为慢跑或奔跑。
* `disableRun`：玩家不能奔跑（按下 Shift 键）。如果玩家尝试奔跑，在允许的情况下，他们会改为慢跑。
* `disableJog`：玩家不能慢跑（这是默认移动速度）。如果玩家尝试慢跑，在允许的情况下，他们会改为奔跑或步行。
* `disableJump`：玩家不能跳跃。
* `disableEmote`：玩家不能自主做表情动作。场景可以在玩家头像上触发动画。
* `disableDoubleJump`：玩家不能进行二段跳。
* `disableGliding`：玩家不能滑翔。

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: false,
		disableWalk: false,
		disableRun: true,
		disableJog: true,
		disableJump: true,
		disableEmote: true,
		disableDoubleJump: true,
		disableGliding: true
	}),
})
```

### 高级语法

要在不使用任何辅助工具的情况下使用该组件，你可以使用以下语法：

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.createOrReplace(engine.PlayerEntity, {
	mode: {
		$case: 'standard',
		standard: {
			disableAll: false,
			disableWalk: false,
			disableRun: true,
			disableJog: true,
			disableJump: true,
			disableEmote: true,
		},
	},
})
```


> [!NOTE]
> **💡 提示**：如需可运行的示例，请参见 [`0,1-input-modifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) 测试场景，其中会分别切换每个 `InputModifier` 移动标志，并展示在移动被阻止时，头像仍会被移动平台带着移动。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
