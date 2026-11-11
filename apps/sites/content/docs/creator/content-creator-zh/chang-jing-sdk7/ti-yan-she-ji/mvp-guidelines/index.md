> 使用 SDK 制作你的第一个 MVP 场景或体验时的推荐指南

# MVP 指南

本文档旨在帮助你引导完成在 Decentraland 中构建场景第一版迭代的过程。我们将把这些称为最小可行产品（MVP）。

**在为你的场景创建最小可行产品（MVP）时，你需要考虑两个关注领域：**

1. 项目的基础用户体验和功能。
2. 创建一个基础“流水线”，或用于构建你的体验并对其进行迭代改进的团队工作流和内容管理系统。

MVP 不应试图展示每一种可能体验的每一种可能结果。相反，MVP 应该是你能够使用 Decentraland 的 SDK 所做出的、对你的体验最好的第一印象。

重要的是要考虑你自身的局限、你计划如何向用户提供内容，以及用户的期望。以这种方式来推进你的 MVP 需要三个不同视角：

1. 作为开发者或制作人，我如何将体验交付给我的用户/玩家？
2. 作为用户或玩家，我对这个体验有什么期待？
3. 作为贡献者或利益相关方，我如何为流水线或体验做出贡献？

区分这种方法与传统敏捷开发很重要，因为你可能不得不使用非最优方法来实现你的设计目标。

你必须在用户期望的背景下审视你自己的目标，以决定某个版本是更侧重玩家、流水线和内容贡献者，还是两者兼顾但程度都不高。

在规划每个版本时，至关重要的是你要根据这三个视角，有意识且审慎地设定优先级。

你可以预期你的开发待办事项会沿着两条线推进：

* 你想创建的用户体验待办列表。
* 构建你的交付流水线所需工具和界面的开发。（或者优化你现有的流水线，使其同时适用于贡献者和你的开发团队。）

这两条线也会遵循两种不同的测试方式：

* 测试你的用户体验更类似于传统的用户界面测试，而且不需要相同的脚本资源。
* 测试你的工具和流水线界面则需要更多技术资源。

你越早把价值主张呈现在用户或玩家面前，就越早能获得反馈来确认或否定该主张。快速确认价值至关重要。许多经验丰富的开发者都会分享这样的故事：他们确信某个新机制会非常惊艳，直到真正用起来才发现它笨拙又有故障，玩家完全没有回应，或者它并不能满足消费者的需求/诉求。你希望以尽可能少的投入快速失败，这样你就能从失败中学习，并规划下一次迭代。

怎样才能快速失败？你只需做到让玩家接触到你的产品所需的最少工作量。

## 最小可行产品的因素

下面是你基础 MVP 需要考虑的因素清单。在开发更稳固的替代方案时，可以先声明你会用某个东西作为占位符，之后再逐步将其替换掉。

1. 美术创作
   * 首先，从基础静态图像开始
   * 你的第一次测试应当针对风格：你选择的风格是否吸引你的用户？
   * 这可以作为提供给外包艺术家的风格指南的起点
2. 场景创建
   * 先建立你空间的基本感觉
   * 玩家应当感觉自己进入了一个新的、独特的空间
   * 将你的空间与相邻空间区分开来
   * 边界是明显可见的——哪怕只是一条画出来的线
   * 用静态内容/美术覆盖整个区域
3. 场景中渲染的美术
   * 使用广告牌是可以的，或使用其他标识（这可以只是实际的广告牌，或者更复杂的朝向摄像机的精灵）
   * 确立你空间的基调和美学（例如：风格、明亮、黑暗）
   * 记录你的流程：美术是如何创建并部署到场景中的？
   * 你希望如何组织你的美术文件，以便重复部署？
4. 玩家体验
   * 玩家能够访问你的空间/场景
   * 玩家可以区分你的空间与相邻空间
5. 流水线目标
   * 部署静态示例场景：不与玩家交互
   * 部署动画场景：如喷泉或飘动的旗帜等元素循环播放动画
   * 部署交互式场景：包含玩家参与
   * 通过重新部署内容来展示部署流水线：从美术创建到场景中呈现，包括脚本 + QA]
   * 暴露流水线缺口：识别特定内容部署领域中的未知项

## 原型的层级

快速失败使你能够通过创建连续的原型来开发你的体验，每次迭代都建立在上一次的基础上。

**从单人玩家原型开始。然后你就可以规划多人交互脚本。最后，你可以处理展示事务层的持久化核心循环。**

**什么是持久化核心循环？**

在游戏设计中，持久化核心循环是驱动玩家行为以及游戏对这些行为作出响应的基础“游戏循环”。这些持久化循环也适用于任何形式的虚拟体验（如 Districts 提供的体验）。

**什么是事务层？**

事务层是系统之间的接口，例如区块链上的更新，或与您的体验连接的其他应用程序，用于维护玩家行为的持久记录。创建并维护这种持久记录，才能构建更个性化的体验。

我们建议将你的 MVP 创建为单人体验。

例如，你可以设计一个包含以下连续体验的场景：

* 单个玩家可以进入世界。
* 玩家可以在场景中与一到两个简单实体进行交互。
* 其他玩家可以加入，并与这个世界以及其他玩家互动。
* 最后，你可以加入一种能力，记住每位玩家进入过场景，并跟踪玩家的事件和活动。

## 如何分享你的 MVP

我们建议在将场景新版本上传到 Decentraland 之前，先与测试用户一起测试更改。你可以将你的场景发布到一个 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 中，通过链接分享，而无需任何 LAND，也不会影响生产中的内容。

## 其他注意事项

一旦覆盖了基本用例，你就可以开始通过聚焦机制来更精细地制定发布管理策略。 **机制** 是一个广义术语，涵盖玩家可以执行的所有操作，以及系统根据这些玩家操作提供的响应。

**设备互操作性** 是需要注意的重要事项。你的场景用户可能会通过桌面设备、移动设备或 VR 头显访问场景。用户应当能够使用任一设备较为顺畅地与场景互动。对于使用 VR 头显的用户，请尽量避免会引发眩晕、导致晕动症的运动。

**音频** 是场景氛围的另一个关键方面。风声、蟋蟀声、远处的交谈，甚至音乐等背景声音，都是增强沉浸感并提供语境的非常强大的方式。你还可以调整音量与声源距离之间的关系，从而让声音位置得到更多或更少的强调。

阅读 [游戏设计约束](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games/) 以详细了解更多其他注意事项。

当你建立好流水线后，可以把 MVP 看作众多原型之一，用它来建立发布节奏。每次发布的重点可能不同，也可能是体验各方面的混合。然而，你应该致力于交付越来越复杂的体验，每次迭代都在前一次基础上构建。

1. **MVP**：单人玩家
2. **版本 2**：增加多人和/或交互支持
3. **版本 3**：引入你的第一个机制
4. **版本 4**：增加音频支持
5. **版本 5**：完成你的美术流水线

例如，假设我们正在为飞盘高尔夫游戏构建一个 MVP。这个 MVP 会包含一些球场的静态图像。玩家甚至可能能够以非常粗糙、积木式的方式投掷飞盘。这让我们能够梳理出基础的投掷机制。下一次版本可能会加入多人支持的原型，这样我们就可以演示并测试两个用户同时登录并在我们的 LAND 上游玩。

记住，虽然最终目标是真正沉浸式的 3D 世界，但你的 MVP 并不是从那里开始的。尽可能快地把玩家带入你的世界应当是你的首要目标。以周而不是月来测试你的版本，对于在不浪费精力的情况下学习和迭代至关重要。

我们强烈建议你时刻关注你的体验所带来的第一印象。一个空空如也的体验会让玩家失望。另一方面，带有一些初始内容和基础体验的场景会向玩家展示未来的潜力，并鼓励他们参与你的社区并回来看接下来的几个版本。

## 需要考虑的持久化因素

最终，你希望达到一种持久化水平，能够证明你架构中的事务层是正常运行的。事务性不仅仅局限于玩家的操作，也包括系统对玩家的响应。

1. **账户信息**：登录名、时区、你特定体验/游戏的位置
2. **排行榜统计**：之前的游戏结果、全球/区域排名、竞赛
3. **身份验证**：Ethereum 钱包地址，或任何其他后端身份管理
4. **区块链更新**：根据你的体验/游戏需要，用于更新区块链账本以实现事务透明性
5. **运行时持久化**：用于在可能分布式的平台上跨会话保留的临时数据（例如，仅针对单个游戏体验的生命值）


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/mvp-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
