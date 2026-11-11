> 设置 Decentraland Rewards 活动：创建活动、添加可穿戴物或表情动作库存、管理资金并激活发放器。

# 快速开始

本页面将引导你使用奖励系统的流程，以激励用户与 Decentraland Wearables 和表情动作的互动。你将学习如何设置和管理奖励活动，包括创建活动、向活动库存添加物品、管理资金以及使用分发器发放奖励。此外，你还会找到有关激活活动和处理交易费用的说明。

## 前提条件

在开始使用奖励系统之前，请确保你已有一个系列 [已获批准并准备铸造](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/fa-bu-he-ji/publishing-collections/)。该系列应包含你计划在活动中作为奖励提供的物品。

## 创建你的第一个活动

活动是指一种旨在通过提供奖励来提升用户参与度的激励计划。本节将介绍活动配置详情，以及开始发放奖励所需的前提条件。

1. 访问 [奖励 dApp](https://decentraland.org/rewards)
2. 连接你的钱包
3. 点击 **活动** 标签页
4. 点击 **创建活动** 按钮。你将被重定向到活动创建表单。

![](/docs/_remote/6288b97cb788a168a929488f.webp)

5. 完成以下字段：

* **活动名称**：为你的活动选择一个名称。该名称将在用户的奖励页面上可见。
* **活动网络**：选择你的可穿戴物品将被铸造所在的网络。请注意，此选择是最终的，活动创建后无法更改。
* **最大 Gas 价格**：设置你愿意为每笔交易支付的最高 gas 价格。如果某笔交易的 gas 价格超过此限制，它将被推迟，直到价格下降。UI 将根据此设置提供每个物品铸造成本的估算。

6. 点击 **创建活动** 按钮。你将被重定向到活动页面。

![](/docs/_remote/2c39dab8808bf032e1c51198.webp)

创建活动后，在开始发放奖励之前还需要完成几个步骤：

* **添加 MATIC 代币余额**：确保你的账户中有足够的 MATIC 代币以支付交易费用。
* **向活动库存添加物品**：将你计划作为奖励提供的物品添加到活动中。
* **创建分发器**：设置一个分发器来负责向用户分发奖励。
* **激活你的活动**：最后，激活你的活动，使其上线并开始发放奖励。

### 添加 MATIC 以支付交易费用

要铸造物品，你需要向活动地址添加一定数量的 MATIC 代币，以覆盖交易费用。你可以在活动名称下方找到该地址。

要从钱包转移 MATIC，首先确保你连接到与活动相同的网络。然后，复制活动地址并将所需数量的 MATIC 发送过去。如果你的钱包中没有 MATIC，你可以参考 [本指南](https://github.com/decentraland/docs/tree/main/player/blockchain-integration/transactions-in-polygon.md#where-can-i-get-matic-to-pay-for-transaction-fees) 来获取一些。


> [!NOTE]
> **💡 提示**：对于运行在测试 **AMOY** 网络上的活动，你可以从 [这个水龙头](https://faucet.polygon.technology/).


要从交易所发送 MATIC，你只需将 MATIC 代币提币到 Polygon 网络（通常交易所不提供 Amoy 网络）。

### 向活动库存添加物品

当你创建新活动时，它一开始是空库存。要开始发放奖励，你需要向活动中添加物品：

![](/docs/_remote/ec2d216749ea170e1a5f766a.webp)

* **授予铸造权限**：首先，授权该活动铸造物品。请遵循 [本指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/fa-bu-he-ji/publishing-collections/#adding-minters-to-the-collection) ，并使用活动地址（位于活动名称下方）作为铸造者地址。提交后，等待交易确认。
* **向活动库存添加物品**：一旦获得铸造权限，你就可以开始向活动库存添加物品。

  a) 点击 **添加库存** 按钮，以查看该活动拥有铸造权限的所有系列。

![](/docs/_remote/0e60e96b9eeeac0ec6fcde02.webp)

```
b) 从系列中选择你想添加到库存中的物品，然后点击 **添加库存** 按钮。
```

![](/docs/_remote/f1349ec843ec2ae7fa0e04a0.webp)

```
c) 选择你想添加到活动库存中的物品数量，然后点击 **添加库存** 按钮。
```

![](/docs/_remote/1281e9ab96332e98d58a7e9a.webp)


> [!NOTE]
> 优先级和分组字段是高级功能，将在专门章节中介绍。


### 创建分发器来发放奖励

分发器是用于从你的库存中发送或领取奖励的工具。每个分发器都关联一个唯一密钥，用于 HTTP API 认证。虽然该密钥不会显示在 UI 中，但你可以通过点击复制按钮来复制它。

当你创建新活动时，系统会自动提供一个带有主密钥的默认分发器，仅用于测试目的。要设置新的分发器，请点击 **添加分发器** 按钮。

![](/docs/_remote/80ef5f28ed3dbc20c04f5aaa.webp)

选择你要从中取出物品的物品分组，然后点击 **保存** 按钮。


> [!NOTE]
> 分发器还有一些配置选项，将在专门章节中介绍。


### 激活你的活动

默认情况下，你的活动处于未激活状态。这会阻止奖励服务分配任何新奖励。要激活你的活动，请点击 **激活活动** 按钮。


> [!WARNING]
> **📔 注意**：如果一个已激活的活动被设为未激活，任何待处理的奖励仍会发送。然而，新的领取请求，或因某些原因需要重试的领取请求，将会被阻止。


**现在你已经准备好开始使用** [**API**](/docs/creator/content-creator-zh/jiang-li/integrations/)

### 管理资金

活动完成后，如果交易费用仍有剩余资金，所有者可以选择收回这些资金，或将其重新分配给另一个活动。要收回资金，只需在活动详情页面点击“管理资金”按钮。

![](/docs/_remote/3ff911494b211a0d72e187c5.webp)


> [!WARNING]
> **📔 注意**：如果活动中的所有资金都被移除，与该活动相关的任何待处理或新的交易都将被阻止，直到分配更多资金。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiang-li/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
