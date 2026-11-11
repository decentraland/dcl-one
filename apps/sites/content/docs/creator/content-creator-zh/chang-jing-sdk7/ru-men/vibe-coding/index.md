> 借助 Creator Hub 和官方 Decentraland SDK Skills（npx skills add decentraland/sdk-skills），与 AI 助手一起构建 Decentraland 场景。

# 使用 AI 进行 vibe coding

通过描述你想要的内容来构建 Decentraland 场景。AI 助手会为你处理 SDK7 代码、ECS 架构和项目结构。

无论你是第一次创作，还是经验丰富的开发者，AI 辅助的“vibe coding”都能让你在几分钟而不是几小时内，从想法变成可运行的场景。


> [!NOTE]
> **💡 提示**：你不需要懂 TypeScript 就能开始。AI 助手可以根据纯文本描述生成可运行的场景代码。


## 什么是 Vibe Coding？

Vibe coding 指的是通过与 AI 助手对话来构建场景，而不是手动编写每一行代码。你只需描述你想要的内容——“一个带可点击门和背景音乐的中世纪酒馆”——AI 就会编写正确、可部署的 SDK7 代码。

这种方法适用于任何技能水平：

* **初学者和非开发者** —— 无需手动编写代码，从零开始就能做出可运行的场景。
* **有经验的开发者** —— 跳过样板代码。让 AI 处理多人同步、UI 脚手架和部署配置，而你专注于创意决策。
* **团队和工作室** —— 在投入完整开发资源之前，快速原型化场景概念。

## 在 Creator Hub 中使用 AI Assistant

最快的开始方式是使用 Creator Hub 内置的 [AI Assistant](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/)。它可以看到你打开的场景，为你编辑实体、组件、智能物件和代码，并且还能运行预览来检查自己的工作。它会自动把下面这些技能安装到你的项目中，因此除了登录一个编码 CLI 之外，不需要额外设置。

这是一个实验性功能，默认关闭。

## 将代码编辑器与 AI 结合使用

使用通用型 AI 代码编辑器，例如 [Cursor](https://www.cursor.com/) ，或者带有 GitHub Copilot 或 Claude AI 的 VS Code。Decentraland 提供了一个上下文文件夹，让这些工具能理解 SDK。

1. 打开 Creator Hub 并创建或打开一个场景。
2. 点击 **< > CODE** 按钮以打开你的代码编辑器。
3. 使用编辑器内置的 AI 助手（Cursor 的聊天、Copilot 等）来生成或修改代码。

## 为任何 AI 代理安装 Skills

Skills 是预先 तैयार好的指令集，用于教你的 AI 代理如何与 Decentraland SDK 协作。每个 skill 都覆盖一个特定主题，比如创建场景、添加 3D 模型或设置多人模式，因此 AI 已经知道正确的模式、API 和限制，而不需要你逐一解释。安装 Skills 意味着更少的错误，以及从第一条提示开始就更好的结果。

```bash
# 从交互式选择器中选择要安装的 Decentraland skills
npx skills add decentraland/sdk-skills

# 或安装所有 Decentraland skills
npx skills add decentraland/sdk-skills --all

# 或选择特定 skills
npx skills add decentraland/sdk-skills --skill create-scene

# 全局安装（在所有项目中可用）
npx skills add decentraland/sdk-skills -g
```

这会把 skill 文件复制到你的代理配置中，让它了解 Decentraland 的模式和限制。

## 更新 Skills

新的 skills 会不断加入，现有的也会持续改进。要获取最新版本，请重新运行安装命令，并加上 `--all`:

```bash
# 更新所有已安装的 skill 并下载任何新的 skill
npx skills add decentraland/sdk-skills --all
```

再次运行 `add` 会重新获取仓库，因此会刷新你已有的 skills，并安装自你第一次安装以来新增的任何 skill。如果你全局安装了 skills，也要把 `-g` 加到这个命令中。


> [!WARNING]
> **📔 注**：不要使用 `npx skills update` 来做这件事。它只会刷新你机器上已有的 skills，因此仓库中在你上次安装后新增的 skills 会被静默跳过。请始终使用 `npx skills add decentraland/sdk-skills --all` 来代替。


## 调试已部署的多人场景

更新后的 `authoritative-server` skill 可以搜索持久化服务器日志、在限定时间内跟踪日志，并在你要求它调查已部署场景时获取 CPU、内存和其他指标。请先更新你的 skills 以及场景的 `auth-server` SDK；有关设置、权限和限制，请参见 [Scene Insights](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/scene-insights/) 。

例如，可以这样要求：

> 调查我已部署的多人场景在过去两小时内的错误以及 CPU 或内存增长情况。使用 Scene Insights，并将日志和指标与源代码关联起来。

你会在浏览器里自行批准钱包登录。之后助手可以复用只读会话；它不需要你的私钥或会话文件。

## 可用的 AI Skills

当你把 skills 安装到你的代理中后，就可以使用以下能力：

| Skill                  | 功能                                      |
| ---------------------- | --------------------------------------- |
| `sdk-scenes`           | 入口点，包含代理指南和所有主题 skill 的索引               |
| `create-scene`         | 从零开始搭建一个新的 SDK7 场景项目                    |
| `migrate-sdk6-to-sdk7` | 将旧版 SDK6 场景迁移到 SDK7                     |
| `add-3d-models`        | 添加 3D 模型（`.glb`/`.gltf`），并进行定位、缩放、碰撞体设置 |
| `add-interactivity`    | 指针事件、触发器、射线检测                           |
| `build-ui`             | 使用 React-ECS 的 2D 屏幕空间 UI——HUD、菜单、对话框   |
| `animations-tweens`    | 使用 Animator、SDK tween 实现 GLTF 模型动画      |
| `multiplayer-sync`     | 使用 CRDT 网络的点对点多人模式                      |
| `authoritative-server` | 用于服务器校验场景的无头多人服务器（BETA）                 |
| `audio-video`          | 音效、音乐、音频流和视频播放器                         |
| `audio-analysis`       | 用于音频响应场景的实时振幅和频率数据                      |
| `deploy-scene`         | 将场景部署到 Genesis City（基于 LAND）            |
| `deploy-worlds`        | 将场景部署到 Worlds（个人 3D 空间）                 |
| `optimize-scene`       | 性能优化、场景限制、最佳实践                          |
| `camera-control`       | 相机模式检测、电影镜头、虚拟相机                        |
| `composites`           | 静态场景内容的复合文件格式参考                         |
| `lighting-environment` | 动态光照、阴影、天空盒、雾效、环境设置                     |
| `particle-system`      | 粒子效果——火焰、烟雾、火花、雪花、烟花                    |
| `npcs`                 | 非玩家角色——NPC Toolkit 库和手动方法               |
| `player-avatar`        | 玩家位置、个人资料、头像自定义、附件                      |
| `player-physics`       | 物理力——冲量、击退、持续力                          |
| `nft-blockchain`       | NFT 展示以及区块链/加密交互                        |
| `advanced-rendering`   | Billboard、TextShape、PBR 材质、视频材质         |
| `advanced-input`       | 系统级输入轮询和玩家移动控制                          |
| `scene-runtime`        | 跨领域运行时 API——异步工作、HTTP、消息传递              |
| `script-components`    | Creator Hub 的脚本组件类                      |
| `game-design`          | 游戏设计模式、场景限制、性能预算                        |
| `unity-explorer-mcp`   | 通过运行中的 Explorer 进行操作，以在世界中测试并验证场景       |

注意：其中一些 skills 会从免费资源目录中获取 3D 模型或其他素材。AI 代理在向场景项目下载任何新资产之前，必须始终先获得用户确认。

## 让 AI 在世界中看到你的场景

通常 AI 会编写代码，而 *你* 运行预览、查看结果并反馈哪里有问题。Decentraland 桌面客户端可以把这个流程闭环：它自带一个可选的 **MCP server** ，让 AI 代理可以直接查看并控制正在运行的 Explorer。代理会自己截图、读取场景的控制台输出、操控玩家在场景中移动、点击对象，并检查场景是否真的实现了它被要求构建的内容——然后修复发现的问题并再次查看。

这会把 vibe coding 从“描述、等待、审查”变成一个代理几乎可以自行运行的循环。


> [!NOTE]
> **💡 提示**：先安装 `unity-explorer-mcp` skill 再尝试这个功能。它会教代理整个工作流程——如何启动客户端、如何构建有用的截图、如何将所见与场景的真实状态交叉核对，以及当场景停止加载时如何恢复。
> 
> ```bash
> npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
> ```
> 


### 你需要什么

* 已安装的 **Decentraland 桌面客户端** （也就是 Creator Hub 启动预览时使用的那个）。
* 一个可以通过 HTTP 连接到 MCP server 的 AI 代理——Claude Code、Cursor、Cline、带有 MCP 支持扩展的 VS Code，以及其他工具。
* 场景中需要有最新的 SDK：运行 `npm i @dcl/sdk@latest` 如果下面的 `--mcp` 标志被拒绝为未知选项。

### 1. 启用 MCP server 启动场景

在你的场景文件夹中：

```bash
npm run start -- --mcp
```

这和 `npm run start` 做的事情一样——把你的场景托管在 `http://127.0.0.1:8000` ，并在你保存文件时自动热重载——另外还会启动桌面客户端，并让 MCP server 监听在 `http://127.0.0.1:8123/unity-explorer-mcp`.

客户端打开后请登录。只有当你通过登录界面并且世界加载完成后，代理才能开始工作。

### 2. 将你的 AI 代理连接到服务器

在 **Claude Code**中，注册一次即可：

```bash
claude mcp add --transport http --scope user explorer http://127.0.0.1:8123/unity-explorer-mcp
```

在 **任何其他 MCP 客户端**中，请按照该客户端文档中的方式添加服务器，并使用以下信息：

| 设置   | 值                                                   |
| ---- | --------------------------------------------------- |
| 传输方式 | Streamable HTTP（不是 stdio——不需要运行命令，服务器存在于正在运行的客户端内部） |
| URL  | `http://127.0.0.1:8123/unity-explorer-mcp`          |
| 认证   | 无                                                   |
| 名称   | `explorer`                                          |

许多客户端会使用 JSON 配置文件（`.cursor/mcp.json`, `mcp.json`，以及类似文件——请查看你的客户端文档以获取准确的键名）：

```json
{
  "mcpServers": {
    "explorer": {
      "type": "http",
      "url": "http://127.0.0.1:8123/unity-explorer-mcp"
    }
  }
}
```

注册服务器后，重启或重新加载你的 AI 客户端，以便它获取连接。如果代理说 Explorer 工具不可用，通常的原因是代理启动时客户端并未运行——请重新连接服务器（在 Claude Code 中，运行 `/mcp`）并打开 Explorer。


> [!NOTE]
> **这安全吗？** 服务器只会在你使用 `--mcp`启动客户端时运行，只接受来自你自己机器的连接（`127.0.0.1`），并拒绝来自网页的请求。没有任何内容暴露到互联网，并且在普通客户端启动时它是完全关闭的。


### 3. 提出你的需求，并让它进行验证

在服务器连接后，你可以像平常一样提出工作要求——不同的是，现在代理可以自行检查结果：

> “在地块中心添加一个宝箱，点击时打开，然后走过去点击它，确认箱盖会播放动画。”

> “霓虹招牌看起来太暗了。截一张图，调整自发光强度，并给我看修改前后对比。”

> “电梯好像有问题。走到平台上，查看日志，并告诉我为什么它不动。”

在幕后，代理可以：

* **查看** —— 截图、读取场景的 `console.log` 输出和错误，检查场景是否加载或崩溃，列出场景实体并检查其组件，以及读取玩家和相机位置。
* **控制** —— 移动和传送玩家，通过真实碰撞沿某个方向行走，调整相机，放置自由相机用于特定镜头，切换相机模式，点击场景对象，发送聊天消息和 `/commands`，触发表情动作，以及重新加载场景。
* **测量** —— 读取场景内容的实时数量（三角形、实体、纹理、材质等），并与 Decentraland 的限制进行对照；从任意视角判断哪些 3D 模型对负载贡献最大；并在玩家所处的任何位置采样真实帧率。

### 用它来优化你的场景

因为代理既能测量也能查看，所以你可以直接把性能工作交给它。它可以将你的场景与 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)进行比较，找出哪些 3D 模型最重，测量特定位置的帧率，最重要的是，在做出更改后重新测量，以证明它们确实有效：

> “检查我的场景是否在 Decentraland 的内容限制之内，并告诉我哪个预算最接近上限。”

> “在喷泉附近场景感觉卡顿。站在那里，测量帧率，并告诉我从那个位置看哪些模型开销最大。”

> “优化场景：找出拖慢帧率的原因，修复它，并给我看修改前后的测量结果。”

为了获得最佳效果，请将 `optimize-scene` skill 与 `unity-explorer-mcp`一起安装，这样代理也会了解 Decentraland 的优化最佳实践。更多关于这个工作流的信息，请参见 [性能优化](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#optimize-with-ai) ，包括如何与 Blender MCP server 搭配使用，以自动修复 3D 模型本身。

### 提示

* **要求证据，而不是口头保证。** “用截图验证”或“从日志确认”正是让这个工作流真正发挥价值的关键。一个好的代理会同时交叉检查两者：像素看起来正确，但底层状态可能已经坏掉，反过来也是如此。
* **截图会消耗 token。** 代理查看的每张截图都会消耗一部分上下文。如果你想要长时间的视觉巡检——例如随时间变化的动画，或对多个位置进行巡查——可以让它把帧保存到文件中，只读取真正重要的部分。 `unity-explorer-mcp` skill 自带一个正好能做到这一点的脚本。
* **保存一次，不要连续保存五次。** 短时间内快速连续保存可能会让客户端加载到一个半写入的 bundle，甚至导致整个场景丢失，这需要重启客户端才能恢复。请让代理把修改批量合并为一次保存。
* **保持客户端打开。** 如果你关闭它，连接就会断开，代理也会失去“眼睛”。用相同命令重新启动就能恢复。
* **传送在本地场景开发中表现不同。** 使用 `/goto` 在地块之间移动在这里是不允许的，因此代理应该在场景内重新定位玩家，而不是传送。

## 有效提示词的技巧

要从 AI 获得最佳结果，关键在于给出清晰、具体的提示词。下面是一些技巧：

### 明确说明你想要什么

不要这样说：

> “让我的场景更好”

试试：

> “在位置 (8, 0, 8) 添加一扇门，点击时通过旋转动画打开，并播放吱呀声效”

### 引用现有物品

> “让桌上的红色按钮触发电梯上升”

### 一次只要求一件事

把复杂请求拆成步骤：

1. “在右上角添加一个记分板 UI”
2. “添加一个在玩家点击目标时递增的计数器”
3. “在记分板上显示计数器的值”

### 迭代并完善

每次修改后：

1. 预览场景（点击 **Play** 在 Creator Hub 中，或 `npm run start` 在命令行中）
2. 检查哪些有效，哪些无效
3. 告诉 AI 需要调整什么：“把 NPC 向左移动 2 米，并让它面向玩家”

### 提示词示例

用于全新克隆的 [sdk7-scene-template](https://github.com/decentraland/sdk7-scene-template/).

#### 提示示例 1

```
我想让你删除当前场景代码，做一个简单的小型迷宫游戏，占用 1 个地块，墙可以用立方体，并且应具有碰撞。

游戏应在玩家真正从 A 点进入迷宫时开始，并在他从 B 点离开时结束（迷宫的唯一出口）。

我想让你在迷宫里添加 3 扇不同的门，需要通过指向输入进行交互才能打开并通过。

你应该验证玩家确实可以游玩并获胜
```

#### 提示示例 2

```
我想让你删除当前场景代码，构建一个简单的平台跳跃游戏。

在认为完成之前，你必须验证游戏确实可以通关。
```

## AI 可以帮助做什么

* 根据描述搭建新场景脚手架
* 添加并放置 3D 模型
* 编写点击处理和交互
* 构建 UI（HUD、菜单、对话框）
* 设置多人同步
* 配置用于反作弊的多人服务器
* 添加音频、视频和流媒体
* 创建动画和 tween
* 优化场景性能
* 为部署准备场景
* 调试现有代码中的问题
* 在运行中的 Explorer 里测试并视觉验证场景（参见 [让 AI 在世界中看到你的场景](#let-the-ai-see-your-scene-in-world))

## 限制

虽然 AI 工具很强大，但请记住这些：

* **始终预览** —— AI 生成的代码可能不会完全符合你的预期。请运行预览进行验证。
* **场景限制仍然适用** —— AI 无法绕过 Decentraland 的 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) （三角形数量、文件大小、地块边界）。
* **复杂的游戏逻辑** —— 对于复杂的游戏机制，你可能需要逐步指导 AI，或手动完善其输出。
* **自定义 3D 模型** —— AI 可以引用现有的免费素材或加载你提供的模型，但它不能从零创建 3D 模型（除非你同时使用其他工具，例如 Blender 官方 MCP server）。

## 下一步

* [SDK 快速入门](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/) —— 学习 SDK7 基础知识
* [与代码结合](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) —— 将可视化编辑与代码结合
* [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) —— 服务器权威的多人模式
* [场景示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/) —— 浏览可复用的场景机制和仓库
* [有用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) —— 更多 AI 工具、素材库和插件，加快你的工作流程


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
