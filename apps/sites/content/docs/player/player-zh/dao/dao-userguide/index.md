> 如何使用 Decentraland DAO

# DAO 用户指南

## 目录

* [登录](#logging-in)
* [投票权](#voting-power)
* [批准/拒绝条件](#approvereject-conditions)
* [浏览提案](#browsing-all-proposals)
* [查看提案](#viewing-a-proposal)
* [将提案添加到你的关注列表](#adding-proposals-to-your-watch-list)
* [投票](#voting)
* [参与论坛](#participating-in-the-forum-discussions)
* [创建提案](#creating-a-proposal)
* [提案类别](#proposal-categories)
* [删除提案](#deleting-a-proposal)

## 登录

要开始使用 DAO，请访问 [governance.decentraland.org](https://governance.decentraland.org)。你将看到欢迎界面和快速入门教程。

阅读快速入门教程后，点击 **登录**。你必须连接钱包才能使用 DAO。目前，你可以使用以下任一钱包： **Metamask** 或 **Fortmatic**。请确保你使用的钱包持有参与 DAO 所需的相关代币（MANA、NAMES 或 LAND）

连接钱包后，你将进入 DAO 首页，其中列出了当前所有活跃提案。

## 投票权

投票权根据与连接到 DAO 的钱包相关联的 MANA、NAMES 和 LAND 总余额计算。 **DAO 同时查看你封装和未封装的余额，因此你无需解封任何 MANA 也能获得完整投票权。** 不过，你可以随时在“投票权”选项卡中解封之前封装的 MANA。由于这是一笔在 Eth 主网上进行的交易，解封 MANA 仍需支付 Gas 费用。

你在 Decentraland DAO 中的投票会根据你登录所用账户关联的 MANA、NAMES 和 LAND 余额加权。

要查看你当前的投票权，请前往 **投票权** 选项卡。

#### 投票权如何计算？

投票权表示为 **“VP”**。MANA、NAME、LAND 和旧版穿戴物会按如下方式为你的总投票权做出贡献：

* 1 MANA 贡献 1 VP
* 1 NAME 贡献 100 VP
* 1 个 LAND 地块贡献 2000 VP
* 以下 Legacy Wearables 系列中，每件 **非凡**贡献 1 VP，每件 **稀有**贡献 5 VP，每件 **史诗**贡献 10 VP，每件 **传奇**贡献 100 VP，每件 **神话**:
  * [社区竞赛系列](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
  * [DCL Public Explorer 上线系列](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
  * [独家面具系列](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
  * [Halloween 2019 系列](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
  * [My Crypto Heroes 系列](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
  * [Xmass 2019 系列](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* 每个 Estate 的价值为 2000 乘以该 Estate 中单个 LAND 地块的数量。例如，一个拥有 2 个地块的 Estate 将为你的总投票权贡献 4000 VP。

#### 如果提案关闭前你的投票权发生变化，会怎样？

如果你的 MANA 或 LAND 余额发生变化，这会影响你的投票权，但仅会影响在你的余额变化之后创建的提案。

在新提案创建的那一刻，DAO 会查看所有投票者的 MANA 和 LAND 余额，以计算他们的投票权。换句话说，当你对某个提案投票时，你这次投票的权重将等于该提案最初创建瞬间你所拥有的余额。你可以使用该 VP 投票，然后再更改你的 MANA/LAND 余额，而不会影响你投票的权重。

#### 为什么 Decentraland DAO 使用加权投票？

如果 Decentraland DAO 给每个以太坊地址一票，并且每票权重相同，那么用户可以随意创建尽可能多的独立地址来获得更多投票权。

目前，通过考虑 MANA、NAME 和 LAND 余额来确定投票权，是限制每位投票者影响力的最安全方式。此外，你在 Decentraland 中拥有的 MANA、NAME 或 LAND 越多，就被认为拥有越大的个人权益，因此你的投票在 DAO 中也更有影响力。

## 批准/拒绝条件

根据提案类型的不同，需要一定数量的参与 VP 才能使投票有效。你会在 Governance dApp 中看到这一点，被标注为该提案的 **接受阈值** 。一旦达到接受阈值，如果支持该提案的总投票权大于反对该提案的总投票权，则该提案会被批准。这就是我们在大多数民主投票中预期看到的典型 50/50 多数模型。

唯一不遵循此规则的提案类型是 **提案前民调**。由于这些提案被视为一种用于收集社区对某个想法反馈的非约束性机制，因此它们没有预设的“是/否”投票选项。用户可以添加两个以上的选项，使其成为多选投票。需要说明的是，如果提案前民调达到定义的 500k VP 接受阈值，它们可能会被升级为一个 **草案提案** 并最终成为具有约束力的 **治理提案**.

## 浏览所有提案

要查看 DAO 中的所有提案，请访问 [governance.decentraland.org](https://governance.decentraland.org)。默认情况下，DAO 首页会列出所有最近添加的提案，并按从最新到最旧排序。

你可以按以下条件筛选提案 **结果**:

* **进行中** – 当前正在投票的提案
* **已通过** – 已被社区批准的提案
* **已拒绝** – 已经投票结束且要么被社区拒绝，要么未达到接受阈值的提案
* **已执行** – 已由 DAO 委员会在链上执行的提案
* **已结束** – 已关闭但没有“是/否”结果的提案，例如多选投票

你也可以按以下条件筛选提案 **类别** ，使用界面左侧的类别列：

* **所有提案** – 无论类别或结果，显示所有提案
* **Catalyst Node** – 仅显示添加新 Catalyst 节点的提案
* **兴趣点** – 仅显示添加新 POI 的提案
* **名称封禁** – 仅显示封禁名称的提案
* **资助申请** – 仅显示资助申请
* **提案前民调** – 仅显示非约束性投票
* **草案提案** – 仅显示草案提案
* **治理提案** – 仅显示最终具有约束力的治理提案

要仅查看已通过的提案，请点击 **已执行** 选项卡。此选项卡中的已执行提案默认按从最新到最旧排序。

## 查看提案

要阅读某个提案，只需点击该提案的标题即可查看详情页。

每个提案详情页都包含提交该提案的人提供的所有描述性信息。

你还会找到该提案在论坛中的讨论线程链接、将提案添加到关注列表的按钮、当前投票结果、投票按钮、接受阈值、你当前的投票权，以及哪些人对该提案进行了投票。

提案详情页还会列出与发起该提案的以太坊地址关联的唯一头像名称（如果存在），以及该提案的开始和结束日期。

最后，你会看到一个指向 Snapshot 上该提案条目的链接——这是 Decentraland DAO 使用的投票平台。

## 将提案添加到你的关注列表

要将提案添加到你的关注列表，请查看该提案的详情页并点击 **添加到我的关注列表**.

要将提案从你的关注列表中移除，请点击 **从我的关注列表移除** ，位于提案详情页中。你也可以点击当前在你关注列表中的任一提案上的红旗图标来将其移除。

## 参与论坛讨论

作为一个治理平台，Decentraland DAO 在与社区频繁讨论相结合时效果最佳。每当在 **governance.decentraland.org**上开启一项新提案时，都会自动在 [**forum.decentraland.org**](https://forum.decentraland.org).

在投票前，请花时间查看并参与这些论坛讨论。点击按钮 **在论坛中讨论** ，位于你想讨论的任何 DAO 提案详情页中。你也可以通过导航到 [forum.decentraland.org](https://forum.decentraland.org)，前往 **治理** 类别，并浏览开放主题。

参与这些讨论不需要持有代币！欢迎每个人为讨论贡献想法。

## 投票

要对提案投票，请登录 DAO： [governance.decentraland.org](https://governance.decentraland.org) 使用持有 MANA、NAME 或 LAND 的钱包。


> [!NOTE]
> **投票所需的最低余额：** 只有 MANA、NAME 或 LAND 持有者才能对 Decentraland DAO 中的提案投票。 **当前在 DAO 中对提案进行加权投票所需的最低余额为 1VP** 以零余额投票将产生权重为 0 的投票，这不会影响最终结果。


要在连接钱包后对提案投票，只需查看该提案的详情页并点击 **投赞成票** 或 **投反对票** 按钮；如果是投票调查，则根据你想要的投票方式选择其中一个多选项。

请务必阅读全文，以便你了解正在讨论的问题，以及如果提案被批准或拒绝会发生什么。

点击投票按钮后，你连接的以太坊钱包会提示你签署这笔交易。请记住，这只是一个已签名的交易，不需要 Gas 费用。

系统会提供将提案添加到你的关注列表的选项，这是跟踪你感兴趣提案的好方法。如果你不想将该提案添加到你的关注列表，只需点击 **不用了**.

提交投票后，你可以在投票期结束前的任何时候更改它——如提案详情页所示。

#### 如果提案完成前你的 VP 发生变化，会怎样？

DAO 会在每个提案创建的那一刻，为该提案单独计算你的投票权。如果你的 VP 在此之后发生变化，则不会影响你在该提案中的投票。

关于投票权以及它何时、如何计算的完整说明，请参阅 [投票权](#voting-power) 用户指南中的该部分。

## 创建提案

要在 Decentraland DAO 中创建新提案，请先登录 [governance.decentraland.org](https://governance.decentraland.org) 并连接一个包含 MANA、NAME 或 LAND 的钱包。

登录并连接钱包后，点击 **提交提案** 并选择你要使用的提案类别。每个类别都会提供一个表单，让你填写与你的提案相关的信息。

每个类别的表单都不同，因此请确保为你的提案选择正确的类别。DAO 中的提案表单支持 Markdown，因此你可以格式化提案内容，使其更易于阅读。这对较长的提案尤其有帮助。

要预览渲染后的 Markdown 文本，请切换 **预览** 开关。如果你不熟悉 Markdown，也可以使用简单的纯文本。

某些提案类别有最低 VP 提交阈值，这意味着你必须至少拥有一定数量的 VP 才能提交提案。治理 dApp 会向你说明这一点。

完成你所选类别的提案表单后，点击 **提交提案**。成功提交提案后，你将进入新提案的详情页，在那里你可以将其加入关注列表并投票。

## 提案类别

### 具有约束力的提案

#### 📍 兴趣点

兴趣点是 Decentraland 中值得注意的位置。这些“POI”会在虚拟世界界面的多个区域中被推荐，并被认为是用户探索的好地方，尤其适合刚接触 Decentraland 的用户。

如果你创建了一个地点，或者发现了一个你认为应该出现在此列表中的地点，或者你认为当前某个 POI 因不再有趣而应该被移除，你可以使用此提案类别向 DAO 提出你的建议。

#### 🚫 名称封禁

“被封禁名称”列表包含在 Decentraland 中不允许使用的冒犯性或有害的头像和地点名称。此列表中的任何名称都不能被认领、使用或在用户之间转移。如需建议封禁某个名称，你可以在 DAO 中使用“名称封禁”提案类别。

#### 🌐 Catalyst 节点

Catalyst 节点是由社区运营的服务器，它们提供内容并建立维持 Decentraland 虚拟世界运行所需的点对点连接。每当用户打开 Decentraland 时，都会连接到这些节点之一。不过，在 Decentraland 网络中使用的只有经过 DAO 批准的节点。

如需建议将新节点添加到网络中，你可以使用 Catalyst Node 类别。

### 治理流程提案

有些提案并不只是简单地在列表中添加或移除某项，它们需要社区表态、讨论和实施路径。这些提案应通过一个三阶段治理流程提交，该流程从投票开始，以具有约束力的提案结束。

投票流程包括三个步骤：提案前民调、草案提案和治理提案。每个阶段的提交和通过阈值都会逐步提高，以确保重要的治理决策由具有代表性的多数（基于投票权）作出。每一步都必须达到定义的 VP 阈值，才能晋级到下一步。

#### 📊 提案前民调

这是进入最终具有约束力的治理提案的第一步。Decentraland DAO 中的投票调查是非约束性的多选问卷，可用于衡量社区对不同问题的一般意见或情绪。它们之所以是非约束性的，是因为 DAO 不会自动根据这些投票调查的结果采取行动。如果提案前民调收集到足够的参与 VP（500k VP 是接受阈值），它可能会被升级为草案提案。

#### 📊 草案提案

草案提案以结构化的格式向社区提出一项潜在政策，并将关于该提案潜在影响和实施路径的讨论正式化。未通过或未达到此阈值的草案提案可以修改后再提交一次。如果草案提案收集到足够的参与 VP（1M VP 是接受阈值），它可能会被升级为具有约束力的治理提案。

#### 📊 治理提案

这是治理流程的最后一步，也是唯一具有约束力的一步。该提案必须详细说明实施此提案所需的所有细节、数据、方法、评估或任何其他相关信息。如果治理提案达到 600 万 VP 的接受阈值，并且“Yes”选项获得简单多数，则该提案会被接受。

## 删除提案

提案只能由创建它的人删除。

要删除你的一项提案，请前往你想删除的提案详情页。点击 **删除提案** 位于右侧栏底部的按钮。你需要确认删除。

请小心！已删除的提案无法恢复！

如果你在有人投票后删除提案，不会发生任何事情。带有约束性操作的已删除提案不会由 DAO 委员会执行。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/dao/dao-userguide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
