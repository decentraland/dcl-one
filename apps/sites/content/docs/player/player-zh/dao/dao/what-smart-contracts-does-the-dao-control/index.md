> DAO 拥有并控制 Decentraland 最关键的智能合约。

# DAO 智能合约

Decentraland DAO 拥有 Decentraland 平台若干最关键的智能合约。如下所示：

## ⛰️ LAND 合约

这是管理 LAND 代币的合约。DAO 是 LAND 智能合约的所有者。这意味着对该合约的任何更改或修改都必须由 DAO 和 SAB 执行。

## 🏘️ Estate 合约

与 LAND 合约一样，DAO 拥有 Estate 合约；在 DAO 通过社区投票批准后，只有 SAB 可以对其进行修改。

## 📍 兴趣点

兴趣点列表（Decentraland 中值得关注的地点，会被推荐给用户作为开始探索虚拟世界的好去处）也归 DAO 所有。该列表存储在一个合约中，只有在社区投票通过后，才可由 DAO 委员会在链上执行修改。

## 🏷️ 名称

用于铸造 Decentraland 中独特头像名称 NFT 的合约由 DAO 拥有并控制。对名称合约的任何更改都必须经过 DAO 批准。

## 🚫 被禁止的名称

从 Decentraland 客户端中被禁用的名称列表存储在由 DAO 拥有的合约中。该列表只有在社区投票通过后，才可由 DAO 委员会在链上执行修改。

## 🌐 Catalyst 节点

用于提供内容并建立维持 Decentraland 虚拟世界运行所需点对点连接的 Catalyst 节点列表，也由 DAO 拥有并控制。该列表存储在一个合约中，只有在社区投票通过后，才可由 DAO 委员会在链上执行修改。

## 👕 可穿戴物品合集

在 Decentraland 中，可穿戴物品在铸造前可以分组为合集。用于管理可穿戴物品合集的合约由 DAO 拥有并控制。

## 🛒 市场合约

Decentraland Marketplace dApp 使用多个智能合约来管理 LAND、Estate 和其他 NFT 的出售与竞价流程。这些合约也定义了市场费用，并且只能在获得 DAO 批准后更改。

## 💰 资助

用于作为 DAO 资助框架一部分、执行定期付款的归属合约也归 DAO 所有。这些合约由 DAO 委员会代表 DAO 创建，并由 SAB 监督，以防止因委员会的漏洞或失误而造成资金损失的风险。任何归属合约的状态都可以使用 [此工具](https://vesting.decentraland.org/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/dao/dao/what-smart-contracts-does-the-dao-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
