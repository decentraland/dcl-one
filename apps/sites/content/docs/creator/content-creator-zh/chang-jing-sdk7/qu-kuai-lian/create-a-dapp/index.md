> 创建你自己的去中心化应用的参考资料

# 创建 dApp

你可以创建自己的去中心化应用（dApp），与 Decentraland 的智能合约交互，并以更丰富、更友好的方式展示其功能。

## 什么是 dApp

去中心化应用，或称 dApp，是运行在分布式点对点网络上的应用，而不是运行在中心服务器上。

在区块链语境中，dApp 使用智能合约，并可能使用 P2P 网络，而不是 Web API 服务。dApp 也可能提供前端，并临时缓存区块链中的信息，但其输出最终会反映在链上。

参见 [本网站](https://blockchainhub.net/decentralized-applications-dapps/) 以了解更完整的 dApp 概览。

## Decentraland 智能合约

Decentraland 编写并维护了若干与 LAND 和 MANA 代币交互的智能合约。

LAND 和 MANA 代币本身由 *LANDregistry* 和 *MANAtoken* 合约分别定义。列表中还包括更具体的合约，例如将多个地块创建为一个庄园，或对地块进行抵押。

* [Decentraland 智能合约](https://contracts.decentraland.org/addresses.json)：你可以找到我们所有合约及其地址的完整列表。

请注意，每个合约在 *主网* 上都有一个生产版本，并在 *sepolia* 上有一个测试版本，而且每个版本的地址都不同。

你可以阅读每个合约的完整代码，因为这在区块链上属于公开信息。例如，你可以在 [Etherscan](https://etherscan.io/contractsVerified) 上按名称找到该合约并查看其内容。

## dApp 样板代码

* [样板代码](https://github.com/decentraland/dapp-boilerplate)：这份样板代码可以作为构建你自己的 dApp 的起点。请注意，该仓库已归档且不再维护，因此请将其视为参考，而不是最新模板。

## 辅助库

在内部构建我们自己的 dApp 时，我们整理了一些你可能也会觉得有用的辅助库。

* [Decentraland-dapps](https://github.com/decentraland/decentraland-dapps)：这是一个更高层级的库，包含适用于 dApp 的常用模块，并被 Decentraland 自己的应用广泛使用。
* [Decentraland UI](https://ui.decentraland.org/)：该库包含一系列可复用的 UI 元素，已纳入 Decentraland 的项目中。
* [Decentraland-eth](https://github.com/decentraland/decentraland-eth)：这是一个低层级库，提供用于操作以太坊区块链的实用函数。请注意，该仓库已归档且不再维护。

## 示例 dApp

下面是我们围绕 Decentraland 构建的多个 dApp 的完整代码链接，这些内容可能会帮助你构建自己的应用：

* [Gate](https://github.com/decentraland/gate)：这个基础 dApp 创建了一个邀请 NFT，在 Decentraland 处于封闭测试阶段时使用。早期，只有持有该 NFT 的白名单玩家才能进入元宇宙。
* [市场](https://github.com/decentraland/marketplace)：这是运行 Decentraland 的完整应用 [市场](https://market.decentraland.org/)。为了让它运行更快，它需要一个数据库和一个连接到以太坊网络的后端服务器，以便基于 LAND 信息创建索引。

## dApp 测试框架

在将你的 dApp 上线到生产环境之前，我们建议先进行测试。

* [dAppeteer](https://github.com/decentraland/dappeteer)：我们构建了这个框架来帮助你对 dApp 进行测试。请注意，该仓库已归档且不再维护。

## Decentraland 智能合约

在以下链接中，你可以找到与 Decentraland 生态系统相关的以太坊智能合约列表。该列表包括主网以及其他以太坊测试网络中的合约。

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/create-a-dapp.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
