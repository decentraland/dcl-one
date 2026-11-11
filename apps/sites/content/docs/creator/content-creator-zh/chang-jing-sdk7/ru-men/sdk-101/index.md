> 使用 Creator Hub、SDK 和官方 SDK Skills 通过 AI 辅助 vibe coding 创建你的第一个 Decentraland 场景。

# SDK 快速开始

本教程将带你创建你的第一个场景，并结合你可用的不同工具：Creator Hub 的可视化场景编辑器、使用 Decentraland SDK 手写代码，以及 AI 辅助的 **氛围编程**。你将把三者结合使用，并了解它们各自最擅长的场景。

## 我该如何创建一个 Decentraland 场景？

1. **先安装** [**创作者中心**](https://decentraland.org/download/creator-hub)，这是用于创建、预览和发布场景的官方桌面应用。它是适合所有技能水平创作者的推荐工具。
2. **如果你使用 AI 编程助手** （例如 Claude Code、Cursor 或 Copilot），请安装官方的 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) ，这样它就能了解已验证的 SDK7 模式： `npx skills add decentraland/sdk-skills`
3. **该** [**CLI**](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/) 是高级用户和自动化工作流的替代方案。

本教程的其余部分将按步骤演示这条推荐路径。

## 安装 Creator Hub

Creator Hub 允许你构建、预览并部署 Decentraland 场景。下载 Creator Hub [此处](https://decentraland.org/download/creator-hub).

要编辑场景代码，你还需要一个代码编辑器。 [Visual Studio Code](https://code.visualstudio.com/) 和 [Cursor](https://www.cursor.com/) 二者都是很好的选择，但任何代码编辑器都可以。

阅读 [安装指南](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/) 以了解更多详情。

## 创建你的第一个场景

1. 打开 Creator Hub。
2. 选择 **场景** 选项卡，然后点击 **新场景**.

   ![](/docs/_remote/efc0e8d68c33532f7a840f5e.webp)
3. 选择一个起始模板。对于本练习，请选择 **空场景**.

这一步可能需要几分钟。它会用基础场景的默认文件集填充你的文件夹。完成后，你会看到场景的空网格。

## 从资源包中添加物品

浏览 **资源包** 在场景编辑器底部区域，并将几个物品拖到你的场景中。现在任何物品都可以。

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

已放置的物品可以点击并拖动来重新定位。参见 [场景编辑器基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#position-items) 以了解更多详情。


> [!NOTE]
> **💡 提示**：用一个地面物体覆盖整个场景。类型为 **地面** 的物品上带有油漆桶图标。将其中一个拖入场景后，它会用该物品的副本覆盖整个场景地面。
> 
> <img src="/docs/_remote/e120bb99677fe9cb301458fb.webp" alt="Ground" data-size="original">


## 运行预览

点击 **播放** 顶部菜单中的按钮，将你的场景加载到 Decentraland 中。现在你可以作为 Decentraland 化身探索这个场景。

![](/docs/_remote/77e30200d3905dd6bed10959.webp)

在工作时你可以保持预览窗口打开：每次修改后它都会更新。更多内容见 [预览场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/).

## 自定义 3D 资源

下载这个牛油果的 3D 模型，格式为 *glb* ，可从以下 [链接](https://github.com/decentraland-scenes/avocado/raw/main/avocado-glb.zip) 下载并解压。

![](/docs/_remote/54ed4744f3c2c873fc597b60.webp)

将 **avocado.glb** 文件从文件管理器拖到场景编辑器底部面板（也就是包含 **资源包** 和 **本地资源** 选项卡的同一面板），然后点击 **导入**.

![](/docs/_remote/63d04fd8ef301b90c747fa37.webp)

现在你可以在 **avocado.glb** 中的模型 **本地资源** 选项卡里，在 **场景** 文件夹中找到它。像资源包中的任何物品一样，把该文件拖到你的场景中。

![](/docs/_remote/23bd93a0c282b7207519dc58.webp)

## 编辑场景代码

点击 **<> 代码** 顶部菜单中的按钮，将你的场景项目打开到代码编辑器中。

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)


> [!WARNING]
> **📔 注意**：如果没有打开，请确保你安装了诸如 [Visual Studio Code](https://code.visualstudio.com/) 或 [Cursor](https://www.cursor.com/) 已安装。


在代码编辑器的左侧边栏，你可以浏览项目的文件和文件夹结构。打开 `index.ts` 位于 `src` 文件夹中的文件。其内容应如下所示：

```ts
import {} from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'

export function main() {}
```

Decentraland 场景是用 [TypeScript](https://www.typescriptlang.org/)编写的，使用 Decentraland SDK：一个库，提供你在场景中放置 3D 内容、添加交互以及控制场景中发生的一切所需的全部功能。

这个文件定义了一个名为 `main()`的函数，它是场景的入口点：你放在这里的任何代码都会在场景首次加载时运行。

你已经通过可视化方式把一个牛油果拖进了场景。现在让我们通过编写代码再添加第二个。这次请将你的 `index.ts` 文件内容替换为以下内容：

```ts
import { Vector3 } from '@dcl/sdk/math'
import { engine, Transform, GltfContainer } from '@dcl/sdk/ecs'

export function main() {
	// 创建一个全新的实体
	let avocado2 = engine.addEntity()

	// 为它添加一个 Transform
	Transform.create(avocado2, {
		position: Vector3.create(8, 0, 8),
	})

	// 为它添加一个 GLTF
	GltfContainer.create(avocado2, {
		src: 'assets/scene/avocado.glb',
	})
}
```

这些行会创建一个新的 [entity](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/)，为它添加一个 [形状](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/shape-components/) ，基于你下载的 3D 模型，并 [设置它的位置](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/) 返回的所有数据，通过 **Transform** 组件，则抛出错误。

通常来说，你在 `index.ts` 中编写的代码都应该放在 `main()` （或者放在 main 间接引用的其他函数中）。参见 [场景生命周期](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#scene-lifecycle) 以了解更多详情。

运行场景预览：你现在应该能看到两个牛油果，一个是你在场景编辑器中添加的，另一个是你通过代码添加的。

![](/docs/_remote/9e7996b651530bdb72380bb3.webp)


> [!WARNING]
> **📔 注意**：第二个牛油果只存在于 **你的代码中**。它会在你运行场景时显示出来，但场景编辑器画布和实体树无法显示在 `index.ts`中创建的实体。如果你在编辑器里看不到它，不必惊慌。当你将可视化编辑与代码结合使用时，这一点非常重要。


这两个牛油果都是由相同的部件构成的。选择 **第一个** 在场景编辑器中的牛油果：属性面板会显示一个 **Transform** 以及一个 **GltfContainer** 组件，其中的字段类型和你刚才在代码里写的一样。它们只是同一事物的两个视图。

![](/docs/_remote/90ea2c66a117f593fd4e77cc.webp)

## 通过 Script 添加交互

让我们让一个牛油果对玩家做出反应。我们可以通过在 `index.ts` 中添加代码来实现，不过这次我们来使用 **脚本组件**：一种将代码直接附加到场景编辑器中某个物品上的方式。它让每个物品的行为保持自包含，而且你甚至可以把同一个脚本复用到多个物品上。

1. 在场景编辑器中，选择第一个牛油果（也就是你拖进来的那个；记住，仅代码创建的牛油果在这里不可见）。
2. 点击 **+** 属性面板顶部的按钮，然后选择 **脚本** 以添加 Script 组件。
3. 点击 **+ 创建新脚本** 并将其命名为 `AvocadoScript`.

![](/docs/_remote/73d08b2cfd32963d68a7790e.webp)

4. 点击 **<> 代码** Script 组件上的按钮，在代码编辑器中打开新文件。

这个脚本是一个类，包含三个主要部分（为简洁起见已删去注释）：

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string, // 请勿删除
		public entity: Entity, // 请勿删除
		// 在下方添加你的自定义输入
	) {}

	start() {
		// 在场景加载时调用一次
	}

	update(dt: number) {
		// 每一帧都会调用
	}
}
```

* 该 **构造函数** 定义会显示在 Creator Hub 的 Script 组件中作为可编辑字段的参数。不要删除 `src` 或 `entity`.
* **`start()`** 运行 **一次**，在场景加载时。可用于初始化：创建组件、注册点击处理器等。
* **`update(dt)`** 运行于 **每一帧** 的游戏循环中，大约每秒 30 次。可用于持续行为，例如移动。 `dt` 告诉你自上一帧以来经过了多少秒。

在这个类中， `this.entity` 始终指向持有 Script 组件的实体：在这里就是你的牛油果。这正是脚本可复用的原因：把同一个脚本附加到十个物品上，每个都会作用于自身。

### 用氛围编程创建你的第一个交互

你可以手动编写下面的代码，但现在正是尝试 **氛围编程**：向 AI 助手描述你想要什么，让它来编写代码。大多数代码编辑器都内置了一个，例如 Cursor 的聊天、VS Code 中的 GitHub Copilot，或者 [Claude Code](https://claude.com/product/claude-code)。你也可以在代码编辑器外编写提示词，比如使用 Claude Desktop 或命令行中的 Claude Code。

在输入第一个提示词之前，请安装 Decentraland SDK skills，这样 AI 就能了解 SDK 的模式，并大大减少错误：

```bash
npx skills add decentraland/sdk-skills
```

这些 skills 是参考文档，当 AI 不确定如何使用 SDK 做某事时会查阅它们，从而编写正确的代码而不是猜测。参见 [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 以了解安装选项和提示技巧。

现在试试这样的提示：

> 在 AvocadoScript.ts 中，让牛油果在玩家点击它时向控制台输出一条消息。

你最终应该得到类似下面这样的内容（或者你也可以自己粘贴进去）：

```ts
import { engine, Entity, pointerEventsSystem, InputAction } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER },
			},
			() => {
				console.log('已点击牛油果')
			}
		)
	}

	update(dt: number) {}
}
```

它在 `start()` 函数中添加了一些点击行为。我们只需要定义一次点击行为，它就会在玩家每次点击该物品时响应。 `pointerEventsSystem.onPointerDown()` 语句定义了三件事：

* 什么 `entity` 点击事件作用于的对象：这里是 `this.entity`，即持有脚本的牛油果。
* 一个 `opts` 对象：要使用哪个按钮，以及其他当前未使用的可选参数。
* 每次实体被点击时都会运行的函数。

要查看记录的消息，请运行预览，并点击 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 右上角的图标打开控制台。你也可以按 **\`** 键来切换显示。每次点击牛油果时，都会看到一行新内容出现：

![](/docs/_remote/609636cba47e8f304bc52855.webp)


> [!WARNING]
> **📔 注意**：要让实体可点击，它必须具有碰撞体几何。这里使用的模型已经包含了一个。参见 [碰撞体](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/) 了解没有碰撞体的模型的替代方案。


### 让牛油果消失

现在让点击做点更有趣的事吧！向你的 AI 助手提问：

> 当牛油果被点击时，让它消失。我希望它以一种带弹跳的缩小动画消失。另外，我希望指向牛油果时看到的提示显示为“Collect”。

你最终应该得到类似下面这样的内容：

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER, hoverText: 'Collect' },
			},
			() => {
				this.collect()
			}
		)
	}

	collect() {
		Tween.setScale(
			this.entity,
			Vector3.One(),
			Vector3.Zero(),
			500,
			EasingFunction.EF_EASEINBOUNCE
		)
	}

	update(dt: number) {}
}
```

让我们来拆解一下 AI 做了什么。

* 为了制作这个弹跳动画，它使用了一个 **补间**。tween 描述的是实体的位置、旋转或缩放随时间逐渐变化的过渡。在这里 `Tween.setScale()` 将牛油果从完整大小（`Vector3.One()`）缩小到零（`Vector3.Zero()`）的过程，耗时 500 毫秒，并使用弹跳缓动曲线。了解更多关于 tween 的内容，请参见 [移动实体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/).
* 它没有直接在点击函数中编写 tween，而是创建了一个单独的 `collect()` 方法。这不是必需的（如果你的 AI 把 tween 放在点击函数里也可以），但这是个好习惯：方法可以让代码更易读，并让你在不同地方复用相同逻辑。
* 为了在牛油果上显示“Collect”提示，它将其设置为 `hoverText` 值。

运行预览并点击牛油果：它应该会优雅地消失。


> [!WARNING]
> **📔 注意**：牛油果会缩小到 0 大小，但实体仍然存在。理想情况下，你应在 tween 结束后删除该实体，以保持场景更轻量。参见 [补间完成时](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/#on-tween-finished).


### 每一帧运行代码

到目前为止，我们的所有代码都运行在 `start()`。让我们使用 `update()` 使牛油果持续旋转。将构造函数和 `update()` 替换为：

```ts
	constructor(
		public src: string,
		public entity: Entity,
		public speed: number = 45,
	) {}

	update(dt: number) {
		const transform = Transform.getMutable(this.entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(this.speed * dt, Vector3.Up())
		)
	}
```

你还需要将 `Transform` 和 `Quaternion` 添加到文件顶部的导入中，使其如下所示：

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction, Transform } from '@dcl/sdk/ecs'
import { Vector3, Quaternion } from '@dcl/sdk/math'
```


> [!NOTE]
> **💡 提示**：与其手动编辑导入，不如点击代码编辑器标记出的错误，让它自动添加。


在每一帧，这都会让牛油果再旋转一点。乘以 `dt` 使动作平滑且与帧率无关：无论玩家机器运行多快，牛油果都会以每秒 45 度的速度旋转。

因为 `speed` 是一个构造函数参数，它也会显示为 Creator Hub 中 Script 组件上的字段。点击 Script 组件右上角的刷新图标即可看到它，然后在不修改任何代码的情况下调整该值。

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="刷新按钮" width="360">

每个实体都会为 **速度** 字段保存各自的值，因此多个物品可以共享同一个脚本但表现不同。试试看：用 Ctrl + C 和 Ctrl + V 复制牛油果，把副本移开让两个不要重叠，然后分别设置不同的速度。现在每个牛油果都会以自己的速度旋转。

脚本还能做更多事情，比如暴露可供其他智能物品触发的动作。参见 [脚本组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/) 了解完整情况。

## 从场景编辑器中引用物品

Script 组件是为单个物品赋予行为的最简单方式，但还有另一种办法：你在 `index.ts` 中可以通过名称获取任何你可视化添加的物品，使用 `engine.getEntityOrNullByName()`。当一段逻辑涉及多个物品，或者你希望所有内容都集中在一个地方时，这很方便。使用显示在 [实体树](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#the-entity-tree).

在这个例子中，我们使用一个名为 **Yellow Crate**。你可以使用任何物品，只需准确写出它在实体树中显示的名称即可。


> [!NOTE]
> **💡 提示**：你可以右键单击实体并选择 **重命名** 来为其改名。


```ts
export function main() {
	// 之前的 avocado2 代码
	// (...)

	const crate = engine.getEntityOrNullByName('Yellow Crate')

	if (crate) {
		pointerEventsSystem.onPointerDown(
			{
				entity: crate,
				opts: { button: InputAction.IA_POINTER, hoverText: '打开' },
			},
			function () {
				console.log('已点击箱子')
			}
		)
	}
}
```

这里 `engine.getEntityOrNullByName()` 会获取一个名为 *Yellow Crate*相同。 `if (crate)` 检查可确保该实体确实存在于场景中；如果没有该名称的实体， `crate` 是 `null`。参见 [参考项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/reference-items/) 了解更多信息。


> [!NOTE]
> **💡 提示**：通过场景编辑器添加的所有实体，在 `main()` 运行时就已经加载完成，因此可以安全地在此处或在其间接调用的函数中引用它们。 `main()`.


## 更多教程

阅读 [场景编码](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/) ，以从宏观层面了解 Decentraland 场景如何运作。

如需查看使用 SDK7 构建的示例，请查看 [示例页面](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)，其中包含可复用的场景和仓库。

参见 **开发指南** 部分，以获取有关向场景添加内容的更多说明。

## 与其他开发者互动

访问 [Decentraland Discord](https://dcl.gg/discord) 以及 [Decentraland DAO Discord](https://discord.gg/bxHtcMxUs4) ，加入关于在 Decentraland Discord 中哪些事情可行以及如何实现的热烈讨论，位于 **创作者** 部分。

要调试任何问题，请查看 [故障排除](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/troubleshooting/) 和 [调试](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview/) 部分。安装了 [SDK skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 的 AI 助手也是一个很好的调试伙伴：把控制台中的错误消息粘贴给它，或者描述哪里没有按预期工作，它通常能找到代码中的问题。如果你没有找到解决方案，可以发帖到 [SDK 支持分类](https://forum.decentraland.org/c/support-sdk/11) 在 Decentraland 论坛上。

## 3D 艺术资源

一个好的体验需要与之相配的优秀 3D 美术。如果你热衷于自己创建这些 3D 模型，请查看 [3D 建模部分](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models/)。但如果你更想专注于编码或游戏设计方面，就不需要自己创建素材！参见 [有用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) ，了解可用于为场景获取 3D 模型的素材库和 AI 工具。

## 发布你的场景

如果你拥有 Decentraland NAME、ETH ENS 名称或 LAND，或者已获得拥有者授予的权限，就可以将你的场景上传到 Decentraland。参见 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/).

## 其他有用信息

* [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)
* [开发工作流](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/dev-workflow/)
* [编辑场景](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)
* [场景编辑器基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/)
* [游戏设计约束](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games/)
* [3D 建模](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models/)
* [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
