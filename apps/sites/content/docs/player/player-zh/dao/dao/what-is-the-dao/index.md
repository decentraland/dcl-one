> DAO 是 Decentraland 的决策平台。

# 什么是 DAO


> [!NOTE]
> **DAO 的使命：** 通过代表性治理、高效管理集体资源和技术创新，确保 Decentraland 生态系统的可持续性。请阅读 [使命、愿景和角色](/docs/player/player-zh/dao/dao/mission-vision-and-roles/) DAO、DAO 理事会、DCL Regenesis Labs 和 Decentraland 基金会的。


Decentraland DAO 是 Decentraland 虚拟世界中 MANA、NAMES 和 LAND 持有者的决策工具。通过 DAO 中的投票，社区可以发放资助，并对被禁止名称、POI 和 catalyst 节点的列表进行更改。DAO 还控制着 LAND 和 Estate 智能合约。

发放资助以及对 DAO 拥有的记录和合约进行更改，只能通过使用预先定义的提案来完成，这些提案可在 [governance.decentraland.org](https://governance.decentraland.org).

这些提案、提交的投票以及最终结果都会通过 Snapshot（一个免 gas 的投票客户端）存储在 IPFS 中。获批且具有约束行动的提案会由一个委员会通过多签钱包在以太坊区块链上执行。该委员会由安全顾问委员会（SAB）监督，SAB 也是一个由受信任密钥持有者组成的多签钱包。该委员会是由社区在 DAO 上一版中投票选出的。 [原始提案可在此处找到](https://forum.decentraland.org/t/proposal-for-a-more-accessible-and-affordable-dao/450).

本文档的其余部分将更详细地解释 DAO 是什么、如何运作，以及可用于什么。

如需了解如何使用 Decentraland DAO 的详细教程，请访问 [DAO 用户指南](/docs/player/player-zh/dao/dao-userguide/).

## DAO 由智能合约驱动

所有 DAO，即去中心化自治组织，都是以太坊所带来的一种新型组织管理和决策方式的一部分。

以太坊通过增加将比仅仅是代币所有权记录更复杂的数据处理去中心化的能力，拓展了区块链的可能性。以太坊通过允许人们在区块链上部署智能合约实现了这一点。

### 什么是智能合约？

智能合约是在以太坊区块链上运行的计算机程序。它既可以存储函数（执行操作的代码片段），也可以存储数据（信息）。智能合约常被比作自动售货机。如果你输入特定的内容，就会得到特定的输出。比如，我走到一台自动售货机前，投进 1 美元，并按下“橙味汽水”按钮，如果机器里还有库存，我就会得到一瓶橙味汽水。如果橙味汽水卖完了，我就会拿回我的 1 美元。

智能合约的工作方式也是如此，人们可以通过发送信息与之交互，并期待获得特定的结果或信息。就像自动售货机里并没有一个小人在里面递出汽水一样，智能合约是自动的（大胆地说，也是自治的）。

如果你想进一步了解以太坊智能合约， [以太坊文档](https://ethereum.org/en/developers/docs/smart-contracts/) 是深入了解的最佳之地。

### DAO 控制着 Decentraland 的关键智能合约

智能合约的第二个重要特性是它们的 **拥有其他智能合约的能力**.

没错，每个智能合约都有自己的地址（就像你的以太坊钱包地址一样），这使它能够拥有其他智能合约和加密货币。

因此，从稍微技术一点的角度来说，DAO 是一个或多个智能合约，它们可以执行特定、预先定义的任务，并持有加密货币的所有权。DAO 的设计方式决定了它们只会在特定条件下执行任务，例如由持有某种代币（如 MANA、NAMES 或 LAND）的一群人投票通过某项提案。所有这些都在区块链上完成。因此，才有了“去中心化自治组织”这个名字。

Decentraland 的 DAO 还拥有一笔 MANA 和其他代币，以及 LAND 和 Estate 智能合约。 [这笔资金](https://governance.decentraland.org/transparency/) 已被预留用于资助社区拨款，并根据社区投票决定的方向帮助发展 Decentraland 平台。


> [!WARNING]
> **📔 注意** DAO 不拥有，因此也无法修改 [MANA 智能合约](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#readContract).
> 
> MANA 合约的所有者是 [TokenSale 合约](https://etherscan.io/address/0xa66d83716c7cfe425b44d0f7ef92de263468fb3d#readContract)。TokenSale 合约的所有者是一个在部署时自毁的独立合约（[（如你在 Etherscan 上所见](https://etherscan.io/address/0xdf861993edbe95bafbfa7760838f8ebbd5afda9f)）。这意味着没有其他合约或钱包拥有修改或暂停 MANA 供应的权限。


DAO 还控制着其他一些信息，例如在 Decentraland 中不允许使用的有害或冒犯性的名称列表、应向新用户推荐的著名地点（POI，即兴趣点）列表，以及托管 Decentraland 虚拟世界的社区运营服务器列表。

转移 DAO 持有的任何 MANA、修改 LAND 或 Estate 智能合约，或修改 DAO 控制的其他任何列出信息 **只能通过** 在获得 MANA、NAMES 和 LAND 持有者的批准后。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/dao/dao/what-is-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
