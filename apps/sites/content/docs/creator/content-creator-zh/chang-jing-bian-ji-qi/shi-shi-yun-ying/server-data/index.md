> 查看和编辑你的场景的多人服务器保存的实时数据。

# 服务器数据

使用某个 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) 可以将数据持久化存储在服务器上：排行榜、玩家进度、环境变化（如门已打开或物品已放置）等。这些数据会被 **实时** 地更新，随着玩家与你已发布的场景互动而变化。

您可以通过网页界面查看所有这些数据，也可以 **直接在那里修改它**。您所做的任何编辑都会立即作用于在线场景，无需重新发布。这包括：

* **场景数据**：所有玩家共享的值，例如排行榜或门的状态。
* **玩家数据**：为单个玩家存储的值，例如他们的进度或偏好。
* **环境变量**：只有服务器才能读取的配置值和机密信息。

## 访问存储界面

您可以直接在以下地址打开存储界面： [decentraland.org/storage](https://decentraland.org/storage).

您也可以从 Creator Hub 访问它：

1. 打开 **管理** Creator Hub 的该部分。
2. 点击 **三个点** 位于您已发布内容的位置旁边。
3. 选择 **查看存储**.

<img src="/docs/_remote/512f80176ec141582be43e70.webp" alt="Creator Hub 中的“查看存储”选项" width="400">

存储界面会显示您已发布场景或拥有操作权限的所有 World 和 LAND 位置列表。打开某个场景即可查看其数据，这些数据按三个选项卡组织： **场景**, **玩家**，和 **环境**.


> [!NOTE]
> **💡 提示**：这些数据仅存在于使用多人服务器并通过以下方式存储数据的场景中： `存储` API。请参见 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) 了解如何进行设置。


## 场景数据

该 **场景** 选项卡列出所有在玩家之间共享的已存储变量，例如排行榜或环境的当前状态。

![Scene data tab](/docs/_remote/76bca9abaef9d73bc207163d.webp)

当玩家与你的场景互动时，此列表中的值会实时更新。您还可以通过单击铅笔或垃圾桶图标来编辑或删除这些变量。更改会被正在运行的场景立即采用，因此这是调整实时值的便捷方式，例如重置排行榜，而无需重新发布。

## 玩家数据

该 **玩家** 选项卡列出所有在服务器上存有任何数据的玩家。您可以通过钱包地址或名称搜索玩家，然后查看其所有关联数据。

与场景数据一样，您可以通过单击铅笔或垃圾桶图标来编辑或删除任何值。

这对于支持工作尤其有用：如果某个玩家报告您的场景中出现问题，您可以查找该玩家并检查其存储的数据，以了解他们的情况。例如，如果他们陷入了不良状态——比如来自旧版本场景的冲突数据——您可以编辑或清除其记录，将他们恢复到稳定状态，而无需重新部署任何内容。

## 环境变量

该 **环境** 选项卡列出为您的场景定义的所有环境变量。这些是只有服务器才能读取的配置值，因此它们是存放 **机密** 信息的合适位置，例如私钥或奖励领取代码：这些值不会经过玩家的机器，也不会出现在您场景已发布的代码中。

![Environment variables tab](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

在此选项卡中，您可以添加新变量，或使用铅笔或垃圾桶图标覆盖和删除现有变量。请注意，您不能 *读取* 变量的当前值；这是有意为之，以保护敏感数据。您只能替换或删除它。

环境变量也非常适合用于功能开关或游戏参数，例如比赛时长或最大玩家数量，您可能希望在不重新发布的情况下在在线场景中进行调整。

## 了解更多

要了解如何从场景代码中读取和写入这些数据，包括随时间更改存储数据结构的最佳实践，请参见 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/server-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
