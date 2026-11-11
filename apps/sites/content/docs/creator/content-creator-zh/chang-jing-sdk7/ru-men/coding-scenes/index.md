> 这组内容将帮助你理解 Decentraland 客户端和 SDK 中的工作方式。

# 编码要点

## 开发工具

从非常宏观的层面来看，Decentraland **软件开发工具包** （SDK）可让你执行以下操作：

* 生成一个默认的 *项目* 其中包含一个 Decentraland 场景，以及渲染和运行你的内容所需的所有资源。
* 在你的网页浏览器中本地构建、测试并预览场景内容——完全离线，无需进行任何以太坊交易，也无需拥有 LAND。
* 使用 Decentraland API 编写 TypeScript 代码，为场景添加交互式和动态行为。
* 将你的场景内容上传到内容服务器。
* 将你的 LAND 代币关联到你上传内容的 URL。

我们的 SDK 包括以下内容：

* **创作者中心**：一个独立应用，除其他功能外，它还让你可以通过简单的拖放界面创建场景。你可以运行预览、调试、编辑代码并发布。 [了解更多](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)
* **Decentraland ECS**：一个包含帮助方法框架的 TypeScript 包，使你能够创建交互式体验。你可以用它来创建和操作场景中的对象，也可用于促进玩家或其他应用之间的游戏内交易。（ [最新 ECS 参考文档](https://github.com/decentraland/ecs-reference/blob/master/docs-latest/decentraland-ecs.md))
* **场景示例**：从以下内容中获取灵感和编码最佳实践： [场景示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/).

其他旧版工具：

* **Web 编辑器**：一个基于网页的工具，用于创建简单场景并发布它们。

## 要求

要在本地开发场景，你不需要拥有 LAND 代币。开发和测试场景可以完全离线完成，无需将场景部署到以太坊网络（Decentraland 用于确立 LAND 和 Decentraland 名称所有权的系统），也无需内容服务器。

你必须具备：

* **创作者中心**：一个独立应用，除其他功能外，它还让你可以通过简单的拖放界面创建场景。你可以运行预览、调试、编辑代码并发布。 [了解更多](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/).

如果你打算编辑场景的代码，还需要安装以下工具之一：

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> **Visual Studio Code**：下载它 [此处](https://code.visualstudio.com/)。它能帮助你更快地编写代码，并减少错误。源代码编辑器会标记语法错误，在你输入时自动补全，甚至会根据你所在的上下文显示智能建议。你还可以点击代码中的某个对象，查看其类的完整定义以及它支持哪些属性。
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> **Cursor AI**：下载它 [此处](https://www.cursor.com/)。一个与 AI 集成的强大代码编辑器。它让你选择不同的 AI 模型来帮助你编写代码，而且全部免费。AI 助手不仅会在你编写时自动补全，你还可以让它重构大型代码库、编写文档等。


> [!NOTE]
> **💡 提示**：你可以使用 Cursor、OpenDCL 或 Claude Code 等 AI 助手，仅凭自然语言描述构建完整场景——无需 TypeScript 经验。参见 [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 开始使用。


## 支持的语言和语法

Decentraland 采用 [TypeScript (.ts)](https://www.typescriptlang.org/docs/handbook/jsx.html) 作为编写场景的默认语言。

TypeScript 是 JavaScript 的超集，所以如果你熟悉 JavaScript，你会发现它几乎一样，但 TypeScript 包含类型声明。得益于类型声明，可以拥有自动补全、更好的调试提示等功能，这些都能加快开发速度，并有助于创建更稳固的代码库。这些功能都是良好开发体验的关键组成部分。

当场景构建完成时，你编写的 TypeScript 代码会被编译成压缩后的 JavaScript，以使其更轻量。TypeScript 的原始源代码不会上传到服务器，上传的只有编译后的 JavaScript 版本。

### 其他语言

你可以使用 TypeScript 之外的其他工具或语言并将其编译为 JavaScript，只要你的编译后脚本包含在一个单独的 JavaScript 文件中，并且该文件匹配在 `main` 字段中为你的场景的 `scene.json` 文件（默认情况下 *bin/index.js*）。所有提供的类型声明均以 TypeScript 编写，其他语言和转译器不受官方支持。

## 场景

你部署到 LAND 的内容称为一个 **场景**场景。场景是一个渲染 3D 内容的交互式程序，它可以是游戏、交互体验、艺术画廊，随你喜欢！

场景会部署到 Decentraland 中的虚拟 LAND。LAND 是一种稀缺且不可替代的资产，由以太坊智能合约维护。部署到单个 **地块**，即一块 16 米 × 16 米的 LAND 地块，或者多个相邻地块。

当玩家访问 Decentraland 时，他们会在地图中行走时下载并渲染每个场景的内容。当他们离开场景时，会卸载这些场景。

你也可以通过 CLI 运行预览，在自己的机器上本地运行一个场景。

## 实体与组件

Decentraland 中的三维场景基于一种 [实体-组件-系统](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) 架构，在这种架构中，场景中的一切都是一个 *entity*。 *组件*每个组件都会为其所属实体赋予特定属性。门实体至少可能有一个 Transform 组件（设置位置、旋转和缩放）以及另一个用于为它提供形状的组件。组件只是存储数据的地方，它们本身不会执行任何操作。

![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

```ts
export function main() {
	// 创建一个实体
	const door = engine.addEntity()

	// 通过 Transform 组件给实体设置位置
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// 通过 GltfContainer 组件给实体设置可见形状
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```

实体可以嵌套在其他实体内部，形成树结构。如果你熟悉 Web 开发，你可以把实体理解为 DOM 树中的元素，把组件理解为这些元素的属性。

![](/docs/_remote/28a71287061591ece4e52409.webp)

实体是一个抽象概念。实体只是一个 ID，用来作为引用以组合不同的组件。

参见 [实体和组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/) 以深入了解这两个概念及其在 Decentraland 场景中的用法。

### 自定义组件

默认的组件集合（例如 `Transform`, `GltfContainer`, `材质`、等等）会被引擎解释，并会直接影响实体的外观、位置、是否发声等。

你还可以定义 *自定义组件* 用于存储可能对你的场景机制有用的数据。引擎不会知道如何解释这些组件上的值含义，因此它们不会对场景如何渲染产生任何直接影响。不过，你可以在场景代码中编写逻辑来监视这些值并做出响应。例如，你可以定义一个自定义的“doorState”组件来跟踪门的开/关状态。在这种情况下，该组件不过是一个用于存储状态值的地方而已。要在场景中看到门打开和关闭，你还必须单独实现逻辑，使用这些值来影响门的旋转——这个值来自于 `Transform` 引擎确实知道如何解释的组件。

参见 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 以了解更多信息。

### 按名称获取实体

在 Creator Hub 的 Scene Editor 中通过拖放添加的实体，也可以通过代码访问，以便进一步编辑并添加行为。

使用 `engine.getEntityOrNullByName()` 通过传入在 Scene Editor 界面中为实体分配的名称来获取实体。每个实体都应有唯一名称。

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```

然后你可以对该实体执行任何操作，比如添加新组件、修改现有组件、复制它或删除它。

参见 [按名称获取实体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#get-an-entity-by-name) 以了解更多信息。

如果该实体是一个 [智能物件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)，你也可以调用它的 **操作** 或订阅其 **触发器** ，可通过代码实现。参见 [参考项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/reference-items/).

## 系统

实体和组件是用于存储场景中对象信息的地方。 *系统* 系统会包含一些函数，这些函数会随时间改变存储在组件中的信息。

系统是我们实现游戏逻辑的地方；它们会执行需要在游戏循环的每个 tick 中定期更新或检查的动作。

系统是一个纯粹而简单的函数，它会按照 [*更新模式*](http://gameprogrammingpatterns.com/update-method.html).

```ts
// 基础系统
function mySystem() {
	console.log('我的系统正在运行')
}

engine.addSystem(mySystem)

// 带 dt 的系统
function mySystemDT(dt: number) {
	console.log('距离上一帧的时间：', dt)
}

engine.addSystem(mySystemDT)
```

一个场景可以同时运行 0 个或多个系统。系统可以在场景生命周期中的不同时间点开启或关闭。通常将彼此独立的行为放在不同系统中是个好习惯。

参见 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 以了解系统在场景中的更多使用细节。

### 游戏循环

该 [游戏循环](http://gameprogrammingpatterns.com/game-loop.html) 是 Decentraland 场景代码的支柱。它会按固定间隔循环执行部分代码，并完成以下操作：

* 监听玩家输入
* 更新场景
* 重新渲染场景

在大多数传统软件程序中，所有事件都直接由玩家操作触发。在玩家点击按钮、打开菜单等之前，程序状态中的任何内容都不会改变。

但交互式环境和游戏并非如此。对场景的变化并不一定都由玩家的操作引起。你的场景可能有会自行移动的动画对象，甚至有拥有自己 AI 的非玩家角色。某些玩家操作也可能需要多个 tick 才能完成，例如如果打开一扇门需要整整一秒，那么随着门移动，它的旋转就必须大约更新 30 次。

我们把循环中的每一次迭代称为一个 *tick*。Decentraland 场景在可能的情况下以每秒 30 个 tick 渲染。如果机器难以渲染每个 tick，更新可能会变得不那么频繁。

在每个 tick 中，场景会更新；然后根据更新后的值重新渲染场景。

在 Decentraland 场景中，并没有显式声明的游戏循环，而是场景的 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 系统

场景的编译和渲染在后端完成，你在开发场景时无需处理这些。

## 查询组件

你可以 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/) 使用方法 `engine.getEntitiesWith(...components)` 以跟踪场景中所有具有某些组件的实体。

通常在一个 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/)中，然后遍历返回的每个实体，并对每个实体执行同样的一组操作。

如果你试图在游戏循环的每个 tick 中遍历场景中的所有实体，那么这会带来很大的性能开销。只引用查询返回的实体，可以确保你只处理相关的实体。

```ts
// 定义一个系统
function boxHeightSystem(dt: number) {
	// 查询同时包含 MeshRenderer 和 Transform 组件的实体
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.get(entity)
		console.log('一个箱子的高度：', transform.position.y)
	}
}

// 将系统添加到引擎
engine.addSystem(boxHeightSystem)
```

## 场景生命周期

如果你开始直接在 `index.ts`中编写零散的代码行，你的代码可能会缺少一些重要上下文。例如，你会缺少关于玩家实体的信息，或者关于在 Creator Hub 中通过拖放添加的实体的信息。当你的代码行被读取时，这些内容还没有加载。

为避免这种情况，始终建议使用 `main()` 函数（在 `index.ts` 文件中）作为入口点来编写场景的初始加载代码。该函数仅在场景的所有初始上下文都已加载之后才运行，其中包括通过 Scene Editor 界面添加的任何内容。

你可以在 `main()` 函数之外编写代码，当：

* 代码被 `main()`
* 代码定义了一个系统，或向引擎添加了一个系统
* 代码位于一个 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/)


> [!WARNING]
> **📔 注意**：当 async 函数或系统中的代码第一次执行时，场景中的一切都已经正确初始化。
> 
> [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 定义是一个例外，它们必须始终写在 `main()` 函数之外，放在单独的文件中。它们需要在 `main()` 执行之前先被解释。


## 可变性

你可以选择处理组件的可变版本或不可变（只读）版本。组件中的 `.get()` 函数会返回该组件的不可变版本。你只能读取它的值，不能更改其任何属性。

该 `.getMutable()` 函数会返回该组件的可变表示，使你能够更改其值。只有在你打算修改组件时才应使用可变版本。处理不可变版本的组件可带来巨大的性能提升。

```ts
// 获取不可变版本（只读）
const immutableTransform = Transform.get(myEntity)

// 以下写法不起作用：
// \timmutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// 以下代码确实会改变实体的位置
mutableTransform.position.y = 2
```

参见 [可变数据](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/mutable-data/) 以了解更多详情。

## 将这些内容结合起来

该 *引擎* 就是位于 *实体*，和 *组件* 一方面和 *系统* 另一方面之间的东西。

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

场景中组件里存储的所有值，代表了该时刻场景的状态。每个游戏循环 tick，引擎都会运行各个系统的函数，以更新组件中存储的值。

在所有系统运行完毕后，每个实体上的组件都会有新的值。引擎渲染场景时，会使用这些新的更新值，玩家将看到实体变化以匹配它们的新状态。

```ts
export function main() {
	// 创建一个实体
	const cube = engine.addEntity()

	// 通过 Transform 组件给实体设置位置
	Transform.create(cube, {
		position: Vector3.create(5, 1, 5),
	})

	// 通过 MeshRenderer 组件给实体赋予一个可见形状
	MeshRenderer.setBox(cube)
}

// 定义一个系统
function rotationSystem(dt: number) {
	// 查询同时包含 MeshRenderer 和 Transform 组件的实体
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.getMutable(entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(dt * 10, Vector3.Up())
		)
	}
}

// 将系统添加到引擎
engine.addSystem(rotationSystem)
```

在上面的示例中， `立方体` 实体和一个 `rotationSystem` 系统被添加到引擎中。该 `立方体` 实体具有一个 `Transform`，以及一个 `MeshRenderer` 组件。在游戏循环的每个 tick 中， `rotationSystem` 系统都会被调用，并更改 `Transform` 组件中的旋转值。 `立方体` 实体的父级。

请注意，上面的大部分代码只会在加载场景时执行一次。例外是 `rotationSystem` 系统，它会在游戏循环的每个 tick 中被调用。

## 场景解耦

你的场景不会在与引擎相同的上下文中运行（也就是主线程）。我们设计 SDK 时使其与渲染引擎完全解耦。出于安全和性能方面的考虑，我们这样设计。

由于这种解耦，你的场景代码无法访问 DOM 或 `window` 对象，因此你无法访问诸如玩家浏览器或地理位置之类的数据。

这种解耦通过 RPC 协议实现，该协议将客户端的一小部分仅用于渲染场景和处理事件。

我们还对通信协议进行了抽象。这使我们能够在 WebWorker 中本地运行场景。

我们不希望开发者干预引擎内部，甚至不需要知道引擎内部有什么。我们需要确保 Decentraland 地图中玩家获得一致的体验，而在这种“底层”更容易出错。

这种解耦对于防止邻近场景在玩家处于别人的场景中时干扰其体验也很重要。玩家可能同时加载多个附近场景，每个场景都运行自己的代码。某些操作（如打开外部链接或移动玩家）只有在玩家站在该特定场景上时才允许执行；如果场景已加载但玩家在场景外，则不允许。

## 树摇优化

当将 TypeScript 源代码转换为压缩后的 JavaScript 代码时，该过程会执行 [树摇优化](https://en.wikipedia.org/wiki/Tree_shaking) ，以确保只转换实际正在使用的代码部分。这有助于让场景的最终代码尽可能轻量。尤其在使用外部库时非常有用，因为这些库通常包含大量未被使用的功能，否则会让场景体积膨胀。

由于树摇优化，你希望场景运行的任何代码，都需要以某种方式被代码的入口点引用： `main()` 函数 on `index.ts`。系统也可以改为在 `index.ts` 文件中添加，而无需引用 `main()`。任何未被这些文件明确或间接引用的代码，都不会进入场景。

例如，假设你有一个名为 `extraContent.ts` 的文件，内容如下，那么该实体不会被渲染，系统也不会开始运行：

```ts
// extraContent.ts

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: { x: 8, y: 0, z: 8 },
})
MeshRenderer.setBox(myEntity)

function mySystem(dt: number) {
	console.log('系统正在运行')
}

engine.addSystem(mySystem)
```

要让它作为场景的一部分运行，你可以通过 `index.ts` 按以下方式引用：

```ts
// 在 extraContent.ts 中

export function addEntities() {
	const myEntity = engine.addEntity()
	Transform.create(myEntity, {
		position: { x: 8, y: 0, z: 8 },
	})
	MeshRenderer.setBox(myEntity)
}

export function mySystem(dt: number) {
	console.log('系统正在运行')
}

/////////////////////////////

// 在 index.ts 中

import { addEntities, mySystem } from './extraContent'

export function main() {
	addEntities()
}

engine.addSystem(mySystem)
```

此规则的例外是自定义组件的定义。这些定义不得通过 `main()` 函数入口点访问，因为它们需要在其他一切之前被解释。

## 导入

场景使用的所有函数、对象、组件和其他元素都必须导入到每个文件中才能使用它们。这是 [tree-shaking](#tree-shaking)的结果，因为它避免打包整个 SDK，而只包含场景使用的部分。

文档中的示例省略了每个文件开头的导入行，以保持整洁，但要让它们正常工作，你必须把它们添加到场景中。

使用 VS Studio Code 编写场景时，智能自动补全选项应该会在你编写时自动帮你处理导入，而无需你意识到这一点。

不过，当你把示例粘贴到场景中时，通常会看到某些元素被标成红色，因为它们尚未导入到该文件中。要修复这一点：

* 点击每个带下划线的单词
* 点击该行左侧的灯泡图标
* 选择 **添加导入自**
* 文件开头会出现一条导入行。

![](/docs/_remote/d183f4383d8bff7a66afa15b.webp)

如果有很多不同内容需要导入，你也可以选择 **添加所有缺失的导入** ，从同一个下拉菜单中。

请注意，元素被使用到的每个文件都必须进行导入。

VS Studio Code 应该能够自行解析导入的正确路径。如果不知为何它在这方面有困难，一个小技巧是在文件开头粘贴以下空导入语句。VS Studio 应该就能继续处理了。

```ts
import {} from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'
```

## SDK 版本

在开发新场景时，默认使用 `@latest` 稳定版 SDK 发布。

如果你想利用或预览尚未进入最新稳定版的即将推出的功能，你可以安装 `@next` SDK 发布版。

为此，请打开你场景的 `package.json` 文件，并修改以下行：

```json
  "devDependencies": {
    "@dcl/js-runtime": "next",
    "@dcl/sdk": "next"
  },
```

然后在你场景项目的文件夹中运行以下命令：

```
npm i
```

参见 [管理依赖](/docs/creator/content-creator-zh/chang-jing-sdk7/ku/manage-dependencies/) 以了解更多详情。


> [!WARNING]
> **📔 注意**：请记住，@next 版本有时可能会出现问题。新功能的语法和名称在其发布为稳定版之前可能会发生变化。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
