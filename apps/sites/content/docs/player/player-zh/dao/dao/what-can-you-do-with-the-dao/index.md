> DAO 允许用户创建提案并对影响元宇宙的提案进行投票。

# 你可以用 DAO 做什么

DAO 允许两类一般提案： **具有直接约束力行动的提案**，以及 **治理提案**.

## 具有直接约束力行动的提案

DAO 允许社区对以下事项进行投票： **具有约束力的行动** 这将导致对以太坊网络上 Decentraland 智能合约的更改。这些具有约束力的行动包括：

* 通过将 DAO 的一部分资源转移到资助归属合约中，为社区项目提供资金。
* 向托管并运行 Decentraland 虚拟世界的服务器网络中添加一个 catalyst 节点。
* 将感兴趣地点（POI）或虚拟世界中的重点位置添加到或移除出一个向用户展示的列表。该列表有助于用户找到受欢迎且有趣的地点进行探索。
* 将一个名称列入 Decentraland 黑名单。此类提案允许用户确保头像不会被赋予冒犯性和有害的名称。

## 治理提案

有些提案并不像从列表中添加或移除某个项目那样简单，它们需要社区信号、讨论和实施路径。这类提案应通过一个三阶段治理流程提交，流程以民意调查开始，以具有约束力的提案结束。

投票流程包括三个步骤：提案前民调、草案提案和治理提案。每个阶段的提交和通过阈值都会逐步提高，以确保重要的治理决策由具有代表性的多数（基于投票权）作出。每一步都必须达到定义的 VP 阈值，才能晋级到下一步。

### 阶段 1：提案前民意调查

* 提交门槛：100 VP
* 通过门槛：50 万 VP（达到至少 50 万 VP 且未获得参与投票权多数支持的民意调查，仍可进入草案提案阶段——确保所有获得足够支持的问题都有一条通往纳入政策的初始路径。）
* 投票期：5 天
* 目标：向社区引入一个治理议题，评估社区情绪，并确定是否有足够支持继续起草初始提案。

### 阶段 2：草案提案

* 提交门槛：1,000 VP
* 通过门槛：100 万 VP 和参与投票权的简单多数（51%）（未通过或未达到该门槛的草案提案，可修改后重新提交一次。）
* 投票期：1 周
* 目标：以结构化格式向社区展示一项潜在政策，并将关于该提案潜在影响及实施路径的讨论正式化。未通过或未达到该门槛的草案提案，可修改后重新提交一次。

### 阶段 3：治理提案

* 提交门槛：2,500 VP
* 通过门槛：600 万 VP 和参与 VP 的简单多数（或其类别所需的接受标准）
* 投票期：2 周
* 目标：将草案提案的通过版本正式确认为具有约束力的治理结果。

需要注意的是，任何达到提交门槛的人都可以将已通过的提案（无论是民意调查还是草案）推进到下一阶段，不仅仅是提案作者。

## 那么，修改 LAND 或 Estate 智能合约又如何呢？

目前，DAO 拥有 LAND 和 Estate 两个智能合约。对这两个合约的任何修改都必须由安全顾问委员会（SAB）和 DAO 委员会来执行——这些是经过信任与选举产生的成员团体，负责确保这些重要的 Decentraland 基础设施组件持续安全。

SAB 必须通过多重签名钱包投票批准对 LAND 或 Estate 合约的任何更改，以防止某个失控的 SAB 成员引入漏洞。

目前，通过 DAO 的界面修改 LAND 或 Estate 智能合约时，并没有预定义的提案类别。这并不意味着非 SAB 成员无法发起更改，但这会是一个更漫长的过程，除了需要提供代码更改所需的技术专长外，还需要获得更广泛社区的支持以及一个可信的第三方来审计这些更改。

LAND 和 Estate 合约的源代码可在 GitHub 上获取 [这里](https://github.com/decentraland/land/tree/master/contracts).

一般来说，修改任一合约的流程如下：

* 通过 DAO 发布一个来征询社区意见 **提案前民意调查**，并公开讨论你拟议的更改以争取支持
* 在获得社区初步支持后，发布一个 **草案提案** 详细说明这些更改，并通过投票获得认可。
* 编写将合并到合约中的更新代码
* 获得信誉良好的第三方成功的代码审查和审计
* 使用具有约束力的方式向社区展示经过审计的代码 **治理提案** 并获得他们的批准，将其合并到合约中
* 在另一轮社区投票批准后，DAO 委员会或 SAB 将使用新代码执行合约升级


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/dao/dao/what-can-you-do-with-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
