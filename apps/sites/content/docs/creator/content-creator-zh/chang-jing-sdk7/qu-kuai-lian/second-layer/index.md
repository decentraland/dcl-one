> 在场景中使用 Polygon 的侧链，以实现更快、更便宜的区块链交易。

# 第二层

## 关于第二层解决方案

任何影响区块链的交易都需要时间才能完成，并且会消耗 gas。这两点都是让区块链游戏流行起来的障碍，因为玩家通常没有耐心等待那么久让自己的操作生效，而且在游戏过程中也不愿意为多笔交易花钱。

一种常见的变通方法是将大部分游戏玩法放在链下，只对关键事件执行区块链交易，例如获得游戏道具或登记高分。

然而，克服这些限制以及区块链本身固有的其他可扩展性问题的另一种方法，是依赖于一个 *第二层* 区块链，也称为 *侧链*.

第二层是位于去中心化应用和主链之间充当中介的另一条区块链。这一层更轻量，因此能够以更低的 gas 成本提供更快的响应。

交易不会直接写入主链，而是在侧链上完成，然后由侧链负责最终将这些变更与主链同步。

侧链能够提供更快的响应，因为它是一个节点更少、规模更小的网络。第二层中执行的交易最初安全性较低，但它们最终会批量提交到主链，而主链的所有安全检查都可以在那里执行。

侧链也能够显著降低单笔交易成本，因为它在与主链同步时会将许多交易合并为一笔。因此，本来在主链上一笔交易需要支付的 gas 可以分摊到数千笔交易上。

此外，在与主链同步时，一些冗余交易可以被完全避免，从而进一步降低交易成本。例如，如果 Alice 向 Bob 转账 1 ETH，然后 Bob 再向 Carol 转账 1 ETH，那么这两笔交易可以简化为一笔，通过记录 Alice 直接向 Carol 转账。

Decentraland 与 [Polygon](https://polygon.technology/)，他们在以太坊上提供自己的侧链。其侧链可供场景使用，以支持快速且极其便宜的交易。

## 试用 Polygon 测试网

要使用 Polygon 的 Matic 测试网，你首先应该具备：

* Sepolia 网络中的 Ether。你可以从各种外部水龙头免费获取，例如 [这个](https://www.alchemy.com/faucets/ethereum-sepolia).

然后你必须配置你的 Metamask 账户以包含 Matic 测试网。

为此：

1. 点击当前网络名称并选择，在 Metamask 中切换网络 *自定义 RPC* 在下拉菜单底部。
2. 将 'Polygon Amoy Testnet' 填写为 *网络名称*, [**https://rpc-amoy.polygon.technology**](https://rpc-amoy.polygon.technology) 作为 *新的 RPC URL* 和 [**https://amoy.polygonscan.com**](https://amoy.polygonscan.com) 作为 *区块浏览器 URL*。对于 *符号* 设置为 **POL**
3. 点击“保存”将此新网络添加到 Metamask。


> [!WARNING]
> **📔 注意**：在他们的 [文档页面了解更多关于如何在 Polygon 上进行设置的信息](https://docs.polygon.technology/)



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/second-layer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
