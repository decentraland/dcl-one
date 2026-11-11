> 为用户提供免成本交易

# 部署你的交易服务器

该 [交易服务器](https://github.com/decentraland/transactions-server/tree/1.8.0) 是一个代理服务器，用于将交易转发到 [Gelato](https://www.gelato.network/relay). 它从客户端接收已签名的交易，然后在后台将其发送到合适的网络。这使服务器所有者能够为用户提供免手续费交易

交易服务器用于改善使用多个网络时的用户体验，并防止用户在运行时切换网络提供商。用户可以保持连接到 [以太坊](https://ethereum.org/en/) 并与 [Polygon](https://polygon.technology/) [只需签署交易即可与](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/)

Decentraland DAO 已设置了一个供我们的 dapp 使用的服务器，在一定限制下承担最多的费用， [限制](#restrictions)。本文档将说明你如何 [部署此服务器](#running-the-server) ，以便在需要时让你的用户能够在你所需的限制下转发交易。

## 限制

所有限制都针对用户尝试发送的每笔交易。实际上，这会转化为向服务器发送一个 POST 请求。

服务器可配置的限制有：

* 检查每日最大交易次数配额。详见 [交易部分](#transactions) 了解更多。
* 检查白名单合约，因此如果交易尝试与一个无法识别的合约交互，它将失败。为此它使用
  * 已部署的合约和集合。详见 [合约和集合部分](#contracts-and-collections) 了解更多
* 销售价格，如果低于阈值则进行限制。详见 [最低销售额部分](#min-sale-value) 了解更多

## 配置 Gelato

[Gelato](https://www.gelato.network/relay) 是一个多链中继协议。我们使用它的基础设施来实现免手续费交易。这实际上意味着，当你要发送一笔交易时，你实际上是在签名一条消息并将其发送给 Gelato。该服务会负责代你发送交易，并将结果（交易哈希）返回给你。

它需要一个用于转发交易的合约，但幸运的是我们可以复用 Decentraland 正在使用的那个（见下文）

[Gelato](https://www.gelato.network/relay) 作为服务器的 API。 [要配置](#configuring-the-server) 它，你首先需要一个 API KEY，我们 [稍后会用到它](#gelato)。获取这些：

* [注册](https://app.gelato.network/relay) 该服务
* 为你想要目标网络创建一个新的 dapp。要模仿 Decentraland 的设置：
  * 选择 `主网` 选项
  * 设置一个合适的 `应用名称` 用于你的 dApp
  * 选择 `Polygon` 作为 `智能合约`
  * 启用 `任意合约` 切换选项
* 从 `API Key` 部分复制 API KEY

最后，你需要为新创建的 dapp 充值。你可以通过在左侧边栏的 [1Balance](https://app.gelato.network/1balance) 部分连接你的钱包来完成。连接后，你就可以存入你的 [USDC](https://polygonscan.com/token/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359) 来资助用户将发送的交易。如果你需要获取 MATIC，请查看这篇 [文章](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/#where-can-i-get-matic-to-pay-for-transaction-fees).

### 测试网

如果你想在上线前测试你的应用，并且你使用的是 Polygon，你可以在 `Polygon Amoy`中进行测试，这是 Polygon 的测试网。

要做到这一点，只需重复 [该流程](#configuring-gelato) ，但将网络字段选择为 `Matic Testnet (Amoy)` 。

你需要为你的 dapp 充值，不过你可以通过从 [水龙头](https://sepoliafaucet.com/).

## 下载交易服务器

首先，你需要一份 Decentraland 的 transactions-server 代码副本。你可以在 [GitHub 上](https://github.com/decentraland/transactions-server/tree/v1)找到它。你有两个选项：

1. **下载代码**：要下载代码，你首先需要点击绿色的 `Code` 按钮，然后选择以下任一方式：

* 点击 `Download ZIP`
* 复制 `Clone` 标题下的 URL，然后运行 `$ git clone THE_URL_HERE`

2. **分叉代码**：你可以点击页面右上角的 `fork` 按钮。过程完成后，你就可以像第一种方式一样下载你的代码。为此你需要一个 Github 账户，更多关于 fork 仓库的信息请参见 [这里](https://docs.github.com/en/enterprise-server@3.5/get-started/quickstart/fork-a-repo)

## 配置服务器

交易服务器使用 [NodeJS](https://nodejs.org/en/) 编写，采用 [TypeScript](https://www.typescriptlang.org/)。在运行之前，你需要配置一些环境变量。

要做到这一点：

* 复制 `.env.defaults` 文件，并将其重命名后粘贴为 `.env`
* 打开 `.env` 文件。你会看到某些变量有默认值，例如 `HTTP_SERVER_PORT=5000` （服务器运行的端口）
* 大多数值可以保持原样，但有几个重要值需要注意：
  * [Gelato](#gelato)
  * [交易](#transactions)
  * [合约和集合](#contracts-and-collections)
  * [最低销售额](#min-sale-value)

### Gelato

使用我们在 [配置 Gelato](#configuring-gelato).

```
时获得的 API KEY：
```

### 交易

当新的交易请求到达时，它会检查金额 **某个地址** 当天发送的数量。如果超过设定值，交易将失败。

```
MAX_TRANSACTIONS_PER_DAY=10
```

要完全移除这一检查，你可以进入代码并删除

```ts
await checkQuota(components, transactionData)
```

方法 `从` 中 `src/ports/transaction/component.ts`

### 合约和集合

服务器将获取合约地址 URL 并将其存储在本地，然后查询 subgraph。当新的交易请求到达时，它会检查交易所交互的合约是否属于 URL 中已部署的合约，或属于 subgraph 中已部署的集合。

如果你想提供自己的合约，请更改 URL 并保持当前的相同结构 <https://contracts.decentraland.org/addresses.json> 的结构。所使用的网络由 COLLECTIONS\_CHAIN\_ID 决定，缓存重新获取的间隔由 COLLECTIONS\_FETCH\_INTERVAL\_MS 决定

如果你有自己的集合，也可以更改 subgraph URL。

要完全移除这些检查，你可以进入代码并删除

```ts
await checkContractAddress(components, transactionData)
```

方法 `从` 中 `src/ports/transaction/component.ts`

```
CONTRACT_ADDRESSES_URL=https://contracts.decentraland.org/addresses.json
COLLECTIONS_FETCH_INTERVAL_MS=3600000
COLLECTIONS_CHAIN_ID=80002

COLLECTIONS_SUBGRAPH_URL=https://subgraph.decentraland.org/decentraland/collections-matic-amoy
```

### 最低销售额

当新的交易请求到达时，它会首先解析它尝试转发的数据。如果检测到销售（市场购买、竞价等），它会将金额与 MIN\_SALE\_VALUE\_IN\_WEI 进行比较。如果更低，交易将失败。

```
MIN_SALE_VALUE_IN_WEI=1000000000000000000
```

要查看相关的销售方法，你可以参见 `src/ports/transaction/validation/checkSalePrice.ts` 并且要完全移除此检查，你可以进入代码并删除

```ts
await checkSalePrice(components, transactionData)
```

方法 `从` 中 `src/ports/transaction/component.ts`

## 运行服务器

现在所有配置都已完成，剩下的就是实际运行服务器。你可以按照 [其 README](/docs/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/deploying-your-own-transactions-server/) 来做，不过简而言之，你需要：

* 拥有 [NodeJS](https://nodejs.org/en/) 已安装
* 打开你喜欢的终端
* 运行以下命令：

```bash
npm install
npm run migrate # 仅在第一次运行时执行
npm run start
```

当然，你大概会想把它部署到你选择的服务上，比如 [AWS](https://aws.amazon.com/) 例如。你可以使用该项目的 [Dockerfile](https://github.com/decentraland/transactions-server/blob/master/Dockerfile) 来实现。

## 使用服务器

现在一切都已设置并运行，是时候真正使用服务器了。

要真正发送一笔交易，你需要向 `/transactions`发送 POST 请求。请求所需的 schema 由 `transactionSchema` 在 `src/ports/transaction/types.ts`.

中定义。 [decentraland-transactions](https://github.com/decentraland/decentraland-transactions)。它通过 [decentraland-dapps](https://github.com/decentraland/decentraland-dapps) 在我们的 dapp 中使用，例如 [Marketplace](https://market.decentraland.org)，配合工具 [`sendTransaction`](https://github.com/decentraland/decentraland-dapps/blob/master/src/modules/wallet/utils.ts#L104)。查看 [这段代码](https://github.com/decentraland/marketplace/blob/a2191515c6ae7ede54a685cc2dd9f9fafa35366b/webapp/src/modules/vendor/decentraland/OrderService.ts#L33) 作为示例。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/deploying-your-own-transactions-server.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
