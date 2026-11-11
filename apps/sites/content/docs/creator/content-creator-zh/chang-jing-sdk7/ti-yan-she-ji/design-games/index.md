> 为 Decentraland 设计游戏时需要注意的事项。

# 设计游戏

本文档介绍了在为 Decentraland 设计游戏时需要考虑的一些关键点。由于其他场景的相邻关系以及 LAND 的分布式所有权等因素，Decentraland 是一个独特的地方，这要求你重新思考你在以往游戏中可能形成的一些假设。

例如，你必须理解，与其他游戏平台不同，Decentraland 中的游戏并不是孤立存在的。你无法控制相邻场景中的内容，也无法控制某些细节，比如玩家的头像或他们可能从其他游戏带入的物品。这带来了令人兴奋的可能性，也要求你以不同的方式思考游戏机制。

目前主流游戏中最接近的例子是 Roblox，在那里，由社区用户生成的内容可以成为其他人探索、游玩和互动的聚集地。与 Roblox 不同，你不是通过浏览一堆互不相关的体验菜单来导航场景，而是通过实际探索一个所有场景彼此相邻的地形来进行。Decentraland 也使用区块链来管理土地、头像、资产等的所有权。

我们一直在持续改进 SDK，因此以下某些限制可能会在未来的更新中被移除。

## 场景边界

**你的游戏必须完全适配场景所建立在其上的 \_LAND**\_\*\*。对于较小的场景，可以把它想象成足球这类游戏：游戏规则会把相关互动限制在一个有限空间内，即使玩家可以走出比赛场地。玩家可以走出场景边界，但任何属于场景的资产或实体都必须留在场景内。

只要仍在可见范围内，走出场景的玩家仍会继续渲染该场景。如果他们走得太远，就会完全停止渲染。

你也可以构建一个横跨多个彼此不连通、且对玩家来说未知的地块的游戏，而对世界其他部分的探索将成为玩法的一部分。这样的游戏将由多个独立的 *场景*组成，这些场景可以通过服务器彼此共享数据。

如果你的游戏发布到 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)，你就能更多地控制玩家在场景边界之外看到什么。World 周围通常会有自动生成的草地、树木和海洋景观，但如果 [禁用此景观](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#landscape-terrain) ，例如你的游戏设定与之冲突时，比如发生在开阔水域或太空中的游戏。

### 用户库存

**目前还没有一个库存系统，供玩家在场景之间行走时存放游戏物品。** 目前可用的替代方案如下：

* 你可以将库存信息存储在场景本身，并将其与每个玩家的 Ethereum 地址关联起来（这可以作为持久 ID 使用）。这些信息只能从你的场景中读取。
* 你可以使用外部自定义存储，并将所有场景与之同步。这是一个更稳健的方案，能够应对更大规模的玩家数量。它也可以将这个库存的访问权限扩展到你或他人拥有的多个独立场景。
* 使用区块链中的代币来处理物品所有权。

当获得某个游戏物品时，它可以作为一个特殊代币存储在玩家的 Ethereum 钱包中。当持有该代币的玩家进入你的场景时，你的场景可以在游戏中赋予该玩家某些属性。

其他场景也可能以不同方式响应同一个代币，这会带来游戏之间有趣的联动。

使用区块链存储库存物品的缺点是，所有交易对玩家来说都有成本，而且不是即时的。关于区块链的更多信息，请阅读下面更专门的部分。

在未来版本中，玩家将拥有一个可随身携带的库存，其中将包含链上和链下资产。

### 可携带体验


> [!NOTE]
> **💡 提示**：见 [可携带体验](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/portable-experiences/) 和 [智能穿戴物](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/) ，了解如何创建它们。


可携带体验是玩家在穿越元宇宙时随身携带的游戏内容的一部分。它们不绑定到土地地块，有时绑定到代币，有时由探索者触发。例如，玩家可以从你的场景中拿起一个雪球，走到另一个场景，然后把雪球扔向另一个也在玩同一游戏的玩家。

智能穿戴物是一种可携带体验，它绑定到一个可穿戴代币，并在玩家穿上该服装时启用。智能穿戴物可以赋予玩家新的能力，比如让他们飞行的喷气背包，或者在世界其他部分之上增加一层新的内容，比如在整个 genesis city 中随机放置可收集的金币。

请记住，在你的场景中，玩家可能正在使用别人的可携带体验。见 [用户数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-portable-experiences) ，了解如何检查玩家当前激活了哪些可携带体验。

## 游戏持久性

**Decentraland 是一个持久化世界，你的场景可以在任何时间被玩家访问。** 你的场景没有启动阶段，也没有结束阶段，因此你应该以一种允许任何时间进出场景的玩家都能参与的方式来设计游戏机制。

你的场景可以有一个重置机制，将其恢复到初始状态，但你应谨慎处理，以免干扰已经在游玩的玩家。

### 同步场景状态

**默认情况下，场景状态不会在玩家之间共享。** 每个玩家都会运行自己本地的场景副本。这是构建场景最简单的方式，但不适合社交体验。

在玩家之间共享状态的最简单方式，是使用 `syncEntity` 函数将实体标记为已同步。参见 [无服务器多人游戏](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/)。采用这种方式时，状态变更不会被存储在任何地方：如果当前没有玩家在场景附近并加载它，那么下次加载时，场景将重置为默认状态。

**你也可以使用服务器来存储有关场景的信息，并让所有玩家与之保持同步。** 这样即使附近没有玩家，场景状态也能保持一致和持久，并且你可以验证更改，从而防止玩家作弊。推荐的方案是使用 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/)，它由 Decentraland 在你发布场景时为你托管和部署。你也可以托管 [自己的服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/third-party-servers/)，这样你就可以把某些信息，比如安全令牌，只保留在服务器上，而不会将这些信息暴露到外部。

### 游戏时序

**使用默认通信架构的游戏应考虑到玩家之间可能会有延迟，** 并且不应依赖不同玩家动作之间的快速反应。我们建议采用回合制游戏，或者主要基于玩家与环境交互的游戏。

对于玩家之间动作时机至关重要的游戏，比如第一人称射击游戏，你应该实现自己的服务器，作为你场景中所有玩家之间实时、权威的事实来源。

## 场景中的玩家

**在 Decentraland 中，玩家通过他们的 Ethereum 钱包地址来识别。** 这个钱包被用作一个持久 ID，并且已经与玩家拥有的所有代币相关联。

**目前还没有办法限制同时出现在 Decentraland 中的玩家数量。** 与许多其他游戏不同，那里可能在不同服务器上托管不同的游戏会话，而 Decentraland 目前所有玩家共享的只有一个实例，至少现在是这样。

你需要记住，在任何给定时刻，都可能有多个玩家在你的场景中走动。其中一些人可能只是路过，并不参与游戏。确保游戏机制不会因此而轻易被干扰。

**你的场景游戏循环不能直接影响玩家**，场景对玩家的动作采取的是响应式方式。如果玩家站在某个实体上，而该实体移动或旋转，玩家也会随之移动。这对电梯、浮动平台等特别有用。

作为场景拥有者，你不能强行把一个违规玩家推出或传送出你的场景。不过，你可以在信号服务器中将玩家列入黑名单。你也可以在场景代码中实现黑名单，并向被列入黑名单的玩家拒绝某些服务。

## 把室内空间建得比现实更大

大多数玩家会使用默认的第三人称摄像机来探索 Decentraland，该摄像机位于头像后方并略高的位置。这个摄像机需要空间。按现实尺寸建造的室内空间在屏幕上会显得像一条狭窄的走廊：摄像机紧贴天花板，头像占满画面，玩家也看不清前方。

把房间外壳建到大约 **现实头顶空间的 1.5 到 2 倍** ：

| 元素          | 现实世界                     | 按这个尺寸建造                        |
| ----------- | ------------------------ | ------------------------------ |
| 天花板高度       | 2.5 到 3 米                | **5 到 6 米**                    |
| 主通道上的门口或开口  | 大约 2.1 米 × 0.9 米         | **大约 4 米高，3 米或更宽**             |
| 走廊宽度        | 1.2 到 1.8 米              | **4 米或更宽**                     |
| 头像必须穿过的任何空隙 |                          | **至少 1.2 米的净空**                |
| 楼梯          | 大约 0.18 米的台阶高度，0.28 米的踏面 | **台阶高度 0.25 米或更低，踏面 0.3 米或更宽** |
| 坡道          |                          | **30 度或更小**                    |

第三人称摄像机围绕头像大约 4.5 米距离处旋转，视野为 60 度。当墙壁或天花板挡在摄像机和头像之间时，摄像机就会向前移动以保持头像在视野中，而这种突然的跳动就是让房间显得局促的原因。因此，玩家会在大约 **9 米宽或更大**的房间中转身时体验最好，也就是摄像机距离的两倍。关于台阶、坡度和头部净空的引擎限制，请参见 [玩家空间限制](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/#player-spatial-constraints).

有几点需要记住：

* **扩大房间，而不是扩大其内容物。** 椅子、桌子、柜台、栏杆和标识应保持接近真实尺寸。如果你把家具和墙壁一起放大，你只是把同样的局促感按更大的尺寸重建了一遍，而头像也不再像是房间里的一个人。
* **在规划地面面积时就应用这一点**，而不是在你放置完所有东西之后。更大的房间需要更多地块，因此这些数字会影响你的 [地块预算](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/).
* **请从正常的第三人称摄像机视角来判断结果。** 自由摄像机或 Creator Hub 视口会让室内空间看起来更好，因为它们可以位于几何体外部并向内观察。请像玩家一样走一遍路线。如果摄像机穿透了天花板，或者头像的头部挤到了画面顶部，那么房间就太矮了。


> [!NOTE]
> **💡 提示**：这是根据实践总结出的设计建议，而不是引擎限制。请把这些数字当作起点，并根据实际视觉效果进行调整。


## 场景内容限制

**请在构建场景时特别注意代码效率。** Decentraland 需要在网页浏览器和移动设备上运行，而玩家在穿越元宇宙时会同时渲染多个场景。

**你还应该尽量保持场景轻量。** 与其他在线游戏不同，在那些游戏中相同的纹理和资产会在大型开放世界中方便地重复使用，而在 Decentraland 中，每个场景都可能拥有完全不同的一组资产。当玩家穿过多个场景时，他们应该能够以合理的速度下载整个场景内容，包括纹理、音频文件等。

因此，我们设置了一些限制，以防止过度使用计算资源。详情请参见 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) ，了解这些限制具体是什么。

## 访问场景

**Decentraland 的地图经过精心设计，设有道路和公共广场，** 这些确保了无论其他人建造什么，地图的各个部分都能轻松到达。不与任何道路或广场相邻的土地地块有可能被邻近场景围墙隔离，不过我们预计大多数场景都应当可以步行到达，并且不会阻挡其他场景。

新玩家会在地图中心的 Genesis Plaza 开始他们的体验，系统会鼓励他们先完成一些教程活动，然后再去探索世界。

玩家也可以手动输入 Decentraland 地图中特定坐标的 URL，以在该位置生成。你也可以分享带有硬编码初始坐标的 URL 链接。

请记住，如果玩家在一个被围墙隔离或低于地形高度的位置开始游戏，那么体验不会太好。为避免这种情况，你可以定义一组场景中的特定位置，作为安全生成点。参见 [场景元数据](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/) 了解详情。

玩家也可以通过地图以及热门地点和活动列表快速浏览世界。你的场景可以包含传送门，将玩家传送到世界的其他部分，参见 [传送门](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/external-links/#teleports).

## 用户界面

**玩家进入 Decentraland 时看到的默认叠加式 UI 只包含最基本的内容。** 当玩家在你的场景中时，你可以在这个 UI 上添加额外元素。请记住，Decentraland 的默认 UI 会显示在你场景中的任何内容之上，因此在设计 UI 时不要与其重叠。

当玩家走出场景时，所有 UI 元素都会被移除，以免干扰其他场景。玩家的屏幕上还提供一个按钮，可切换关闭场景中的所有 UI 元素，这主要用于防止场景出现恶意行为，因为有些场景可能试图覆盖玩家整个视野。

## 物理

请记住，SDK 并不为实体提供完整的物理引擎。你可以对玩家的头像施加力和冲量，参见 [玩家物理](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics/)。对于实体之间的物理效果，比如碰撞或重力，你可以导入一个库，或者自己编写行为。

## 控制器输入

**你的游戏控制应限制在 SDK 支持的一组输入操作内**：移动键、跳跃、指向和点击、主按钮（E）和次按钮（F），以及编号操作按钮（1 到 4）。这些输入是从物理按键抽象出来的，因此也可以映射到移动应用的屏幕控制或其他设备上，不要假设每个人都有键盘。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) 了解完整列表，以及 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/) ，了解面向移动玩家时应优先使用什么。

所有这些输入都支持全局的 *按钮抬起* 和 *按钮按下* 事件，以及命中事件，让你可以识别某个实体是否在玩家的瞄准范围内。

## 头像

**玩家可以从大量可穿戴物品目录中打造自己的头像**，其中包括由社区创作者在 Decentraland Marketplace 上创作和出售的可穿戴物。你的场景可以读取玩家穿着了什么并作出反应，参见 [用户数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data).

## 玩家之间的交流

**玩家可以彼此聊天、进行语音聊天，并通过播放表情动作来传达肢体语言** ，例如跳舞、鼓掌或挥手，包括由社区创作者出售的表情动作。

你的场景可以检测玩家何时播放了某个表情动作，也可以让玩家的头像播放动画。参见 [玩家播放动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-plays-animation) 和 [头像动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/).

## 游戏通知

**目前还没有跨场景通知系统。** 任何需要在当前场景之外显示通知的游戏，都必须使用外部服务来实现。

## 使用区块链

**在 Decentraland 中，区块链用于存储所有权信息。** 目前这主要指 LAND 所有权，但它也可以用于游戏物品、可穿戴物、特殊头像、表情动作以及代币的所有权，这些都可以确保某些游戏权限或游戏访问权。

区块链并不用于存储游戏状态、玩家位置或任何需要实时变化的内容。

### LAND 和 MANA

**玩家无需拥有任何土地地块即可参与元宇宙。** 事实上，绝大多数玩家都不会拥有。玩家头像和他们拥有的 LAND 代币之间没有任何直接关联。

**玩家在进入 Decentraland 之前，无需事先拥有 Ethereum 钱包或 MANA 代币。** 如果你的游戏玩法高度依赖代币所有权，那么你会排除大多数玩家。免费增值的游戏模式可能是同时满足这两类用户群体的理想方式。

### 其他 NFT

**你可以使用特殊的非同质化代币（NFT）来表示游戏物品、自定义头像或可穿戴物。** 如果玩家拥有这些代币之一，你的场景可以以不同方式响应它。

了解 NFT 是什么，请看 [这篇博客文章](https://decentraland.org/blog/technology/what-are-nfts/).

### 游戏内交易

**你的场景可以支持区块链交易，让玩家购买或赚取代币。**

区块链交易不是即时的，它们需要验证时间，并且会产生 Ether 成本，时间和费用都会因当前网络使用情况而变化。

Decentraland 使用 Polygon 侧链来让交易比 Ethereum 网络更快、更便宜。这个侧链非常适合游戏内交易，因为更改可以接近实时发生，而且成本很低。对于需要更高安全性且可以接受更高费用和更长时间的交易，仍然推荐使用主 Ethereum 链。参见 [第二层](/docs/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/second-layer/).

玩家必须始终在其 Ethereum 客户端中明确批准这些交易。例如在使用 Metamask 时，Metamask 会在每笔交易处理之前提示玩家接受。

玩家还可以签署一个合约，使其自动批准由特定地址请求的所有交易，或在某些限制条件下自动批准交易，从而避免为批准交易而中断流程。

你也可以使用智能合约根据自定义条件来约束交易。例如，玩家可以对某个游戏结果下注，而一旦结果已知，对应的支付就会自动发生。

要在你的场景代码中实现区块链交互，你必须使用与 Ethereum 网络接口的外部库。SDK 的未来版本将提供自定义 API，以更简单的方式暴露这些功能。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
