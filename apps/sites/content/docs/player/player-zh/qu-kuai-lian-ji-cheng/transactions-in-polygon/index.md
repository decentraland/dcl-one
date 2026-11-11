> 关于 Polygon 交易的常见问题

# Polygon 上的交易

## 什么是 Polygon？

正如 Polygon 官方网站所述， [Polygon](https://polygon.technology/) 是一个用于构建和连接与以太坊兼容的区块链网络的协议和框架。汇聚以太坊上的可扩展解决方案，支持多链以太坊生态系统。

## 什么是 Matic？

MATIC 是 Polygon 的原生代币。它是一种加密货币，用于支付 Polygon 网络中的 gas 费，等等。MATIC 之于 Polygon，正如 ETH 之于以太坊。

你可以在大多数加密货币交易所购买 MATIC。

## Decentraland 的 Marketplace 和 Builder 中的 Polygon

通过使用 Polygon 网络，并得益于 Decentraland 的 DAO，用户可以在 Marketplace 上上架、出售和购买可穿戴物，或在 Builder 中发布他们的收藏，而无需使用元交易服务支付交易 gas 费。

**Polygon 上的交易并非免费**。Decentraland DAO 会承担 Polygon 上的交易成本，因此用户可以在 Marketplace 中享受许多免手续费交易。

若要享受免手续费交易，需要满足三个条件：

* 你需要连接到以太坊主网。
* 你打算购买的物品价格需要为 1 MANA 或更高。
* 你还没有达到免费交易额度上限。

当网络 gas 费过高时，Decentraland DAO 保留消耗或暂停元交易服务的权利，以防止 gas 池过快消耗，使其持续更久，并能供尽可能多的人使用。

## 为什么我必须为低于 1 MANA 的物品承担交易费用？

为了避免被滥用并保护免费 gas 服务，价格低于 1 MANA 的物品不包含在免手续费交易中。你可以通过连接到 Polygon 网络，并在钱包中持有 MATIC 来购买这些物品。

gas 费用是浮动的。

## 如果网络费用高于 300 gwei，我该怎么办？

Gwei 是以太坊区块链中使用的计量单位，是 “Giga-wei” 的缩写，其中 “giga” 表示十亿。在 Polygon 中，gwei 是 MATIC 的最小单位，等于 1 MATIC 的 1/1,000,000,000（1 MATIC = 1,000,000,000 gwei）。Polygon 网络上的所有交易费用都以 gwei 计价。

网络费用（以 gwei 表示）是浮动的，所以你能做的最好办法就是等待，稍后再试。 [这里](https://polygonscan.com/gastracker) 你可以实时查看 Polygon Gas Fee。

另外，你也可以通过连接到 Polygon 网络，使用钱包中的 MATIC 支付费用来购买这些物品。MATIC 费用会作为 Polygon 网络交易费用的一部分自动扣除。你只需要连接到 Polygon 网络，并在钱包中持有 MATIC。

gas 费用是浮动的。

## 如果达到免费交易额度会怎样？

免费交易额度每天都会刷新，所以你可以第二天再试。或者，你也可以在连接到 Polygon 网络时使用 MATIC 支付交易费用。

MATIC 费用会作为 Polygon 网络交易费用的一部分自动扣除。你只需要连接到 Polygon 网络，并在钱包中持有 MATIC。

gas 费用是浮动的。

## 我可以从哪里获取 MATIC 来支付交易费用？

获取 MATIC 的一种方式是通过……购买 [Account dapp](https://account.decentraland.org/).

只需点击 Polygon MANA 区域中的 BUY 按钮，即可将你想购买的加密货币从 MANA 兑换为 MATIC。

另一种方式是通过去中心化或中心化交易所。你可以查看 <https://polygon.technology/matic-token/> 以查看哪些交易所支持 MATIC。如果你想使用中心化交易所，请确保它支持通过 Polygon 网络提币。

如果你在以太坊网络中持有 MATIC，你始终可以使用 [Polygon Bridge](https://wallet.polygon.technology/bridge/) 将这些 MATIC 存入 Polygon 网络。


> [!WARNING]
> **警告：** 在 Polygon 中完成交易后，请记得切换回以太坊主网，以便享受 Marketplace 和 Builder 中不受 Polygon 网络支持的所有功能。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/qu-kuai-lian-ji-cheng/transactions-in-polygon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
