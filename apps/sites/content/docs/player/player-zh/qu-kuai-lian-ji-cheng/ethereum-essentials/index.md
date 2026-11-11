> Decentraland 如何使用 Ethereum 区块链。

# 关于区块链

所有区块链本质上都是分布在网络中的各台机器上的去中心化数据库。交易会被分组为“区块”，并按顺序处理，从而形成一个 *链* 事件序列。

以太坊是最受欢迎的区块链之一。它与比特币等其他区块链不同之处在于，它把区块链作为存储介质，存放的不仅仅是货币交易记录。以太坊可以存储更复杂的信息，用来区分不同类型的代币，甚至处理具有特定特征的独特代币。以太坊区块链还运行智能合约，这使得可以执行更复杂的交易，这些交易也可以依赖于已约定的事件。

Decentraland 使用以太坊区块链记录数字资产以及其他可交易物品的所有权，这些内容可以被 3D 场景读取并做出响应。

区块链不会用于存储场景状态、玩家位置，或任何需要在玩家与场景交互时实时变化的信息；这些内容要么存储在每位玩家机器上的本地，要么存储在场景所有者拥有的私有服务器上。每个场景的开发者必须选择哪些信息值得存储在区块链上，以及哪些信息应存储在私有服务器中。

## 钱包

以太坊代币由钱包持有。一个以太坊钱包可以持有多种代币，包括 Ether、MANA、LAND，以及其他可用于 Decentraland 游戏或体验的代币。

有许多钱包提供商可供你持有 Decentraland 代币。要使用市场，或进入 Decentraland，你必须使用与你的网页浏览器集成的钱包，因此我们推荐你使用：

* [Metamask](https://metamask.io/)
* [Trezor](https://trezor.io/)/[Ledger](https://www.ledger.com/) 硬件钱包

每个钱包都有公钥和私钥。你的公钥哈希就是你钱包的唯一地址，用于路由交易并识别玩家。你的私钥由钱包用于对你发送到网络的每笔交易进行签名，并证明该交易确实由你发出。你的私钥也用于在你忘记密码时恢复钱包，因此请把它保存在安全的地方，不要与任何人分享。

在 Decentraland 中，玩家身份围绕钱包构建。由于钱包公钥是唯一的，你的场景可以用它们以持久方式识别 Decentraland 用户。钱包还可以持有不同的代币，这些代币可以为玩家提供独特的头像、可穿戴物品、进入限制访问场景的权限、在游戏中使用的特殊武器等。

## 交易

交易会改变存储在区块链中的信息。典型的交易涉及代币所有者的变更，例如用户 A 将其 LAND 代币转给用户 B，以换取一定数量的 MANA 代币。不过，在以太坊网络中，交易也可以表示在不改变代币所有者的情况下，修改有关该代币存储的信息。例如，修改某个地块的描述，或将多个地块合并为一个 Estate。

发生在以太坊主链上的所有交易都有成本，这些成本以 Ether 代币支付。该费用被称为“gas”费用，并支付给“挖出”该交易的网络用户。

当你请求进行一笔交易时，你会设置愿意为该交易被挖出而支付的 gas 价格。出价更高的交易会更快被挖出，因为矿工会优先处理它们。这些交易的市场价格会定期波动，当网络使用率更高时，价格通常也会更高。请确保你提供的价格不低于市场价，否则你的交易可能会无限期地停留在未处理队列中。

所有交易都必须由以太坊地址使用该地址的私钥进行签名。这证明了该交易是由该地址执行的。

#### 交易验证

区块链交易并不是即时的，它们需要时间由网络中的某个节点“挖出”，然后再传播到其余机器上。网络请求的交易越多，验证所需的时间就越长。

简而言之，交易的验证流程如下：

1. 一笔新交易发生后，它会进入未确认交易池。
2. 网络中的一台机器成功解出一个算法，挖出一个包含该池中若干交易的新“区块”，其中包括这笔交易。它将这个新区块附加到链的末尾。
3. 该区块会与网络中的其他机器共享。每台机器都会验证区块中的每笔交易是否有效，并检查区块的哈希值以确保其合法，然后将其加入自己版本的链中。
4. 新区块会传播到整个网络。大家都会达成一致，认为这笔交易已经发生。

#### 侧链

Decentraland 正在与 [Matic](https://matic.network/) 合作创建一个 *侧链* （一种特殊的区块链），它能够比以太坊主网络更快、更便宜地处理交易。这个侧链非常适合游戏内交易，因为变化可以更接近实时发生，而且成本非常低。对于涉及高价值物品的交易，我们仍然建议使用以太坊主链，因为它会更安全。

每位正在开发场景的开发者都可以为不同的交易选择使用主链、侧链，或者两者结合。

该侧链将与以太坊主链保持互操作性。你可以将主链中的代币加载到侧链中，反之亦然。当代币“退出”并返回主链时，发生在侧链中的交易最终会反映到主链上。

#### 从场景中触发交易

你场景中的代码可以触发交易，既可以在以太坊主链上，也可以在 Decentraland 的侧链上。你可以在场景中开设一家商店出售代币（如 NFT），或者做一个游戏，在玩家达成特定目标时奖励游戏物品。

用户必须始终在其以太坊客户端上明确批准这些交易。例如，在使用 Metamask 时，Metamask 会在每笔交易处理前提示用户接受。

## 代币类型

以太坊网络可以处理不同类型的代币。已经出现了一些标准，用于归类具有相同特征的代币。

在 Decentraland 中，你可以用代币来表示与游戏或体验相关的物品，例如武器或奖杯。由于代币存放在玩家的钱包中，它们会随着玩家从一个场景带到另一个场景，因此每个场景都可以选择是否以及如何对每一种现有代币做出反应。

阅读 [什么是 NFT](https://decentraland.org/blog/technology/what-are-nfts/) 我们博客中的这篇文章，了解非同质化代币的兴起与演变。

#### 同质化代币

如果一种物品是同质化的，那么它可以被任何类似物品替代或交换。法定货币，如美元，就是同质化的。一美元纸币可以与任何其他一美元纸币互换。

像 Bitcoin、Ethereum 和 MANA 这样的加密货币代币都具有同质性，因为一个代币单位可以与任何其他代币单位交换。

你还可以创建自定义的同质化代币，用于 Decentraland 场景，并用它们来描绘那些完全相同、彼此之间没有任何区别或可定制属性的物品。比如，你可以创建一个围绕收集大量相同物品展开的游戏，并用同质化代币来表示这些物品。你也可以使用同质化代币来表示一张金券，让持有它的玩家有权进入某个特定区域或使用某项服务。

*ERC20* 是以太坊网络中最被广泛接受的同质化代币标准。MANA 就是基于这一标准构建的。

#### 非同质化代币

非同质化代币（NFT）具有使每个单位在客观上都与其他单位不同的特性。Decentraland 中的 LAND 地块就是 NFT，因为每个地块的位置都是唯一的。与其他地块、道路或区域的邻接关系，使这些位置对代币所有者具有重要意义。

在 Decentraland 中，你可以使用 NFT 来表示游戏内物品，例如头像、可穿戴物、武器以及其他库存物品。例如，你可以用同一种 NFT 来表示游戏中的所有武器，并通过为这些 NFT 设置不同属性来加以区分。

NFT 可用于提供可被证明稀缺的数字商品。由于区块链带来的真实稀缺性，买家可以放心，他们购买的艺术品确实是稀有的。这赋予了数字艺术前所未有的真实价值。

游戏物品会有记录在区块链中的历史。这样的历史可能会使某件物品更有价值，例如它曾被用来完成重大成就，或者曾被备受敬重的人使用过。

根据描述该代币的合约，每个 NFT 都可以是不可变的，或者你也可以允许玩家在选择时自定义并更改其某些特征。

*ERC721* 是以太坊网络中最被广泛接受的非同质化代币标准。LAND 代币遵循 ERC721 标准。

## 智能合约

合约由代码（其方法）和数据（其状态）组成，并驻留在以太坊区块链上的某个特定地址。

合约中的方法总是通过一笔交易来调用，这笔交易的 *到* 字段被设置为合约地址。合约方法执行的代码可以包含对其他合约的调用，这些调用会触发更多交易，而这些交易的 *从* 字段被设置为合约地址。

合约不能自行触发任何操作，也不能基于时间事件触发操作。智能合约执行的所有动作都始终源自某笔调用该合约某个函数的交易。

你可以使用智能合约，根据自定义条件来约束交易。例如，玩家可以就某个游戏结果下注，而一旦游戏结果被告知合约，对应的付款就会立即发生。

智能合约的全部代码对任何想阅读它的人都是公开的。这使开发者能够创建可公开验证的规则。

所有代币都由智能合约定义，该合约规定了它的特征以及可以对它做什么。Decentraland 已编写并维护了多个智能合约。LAND 和 MANA 代币本身分别由 *LANDregistry* 和 *MANAtoken* 合约定义。

你可以在 [Decentraland 智能合约](https://contracts.decentraland.org/addresses.json).

中找到 Decentraland 创建的每个合约的地址。你可以阅读这些合约的完整代码，因为它们是区块链上的公开信息。你可以在 [Etherscan](https://etherscan.io/contractsVerified) 上按名称查找合约并阅读其内容。

## dApp

*dApp* （去中心化应用）是建立在智能合约和区块链之上的应用程序。

dApp 可以很简单，只需验证你的钱包是否持有某个代币并允许你使用某项服务。它也可以是一个功能完整、拥有自身界面的应用程序，例如 Decentraland Marketplace。

## Sepolia 测试网络

在你部署智能合约、创建新类型的代币，或创建依赖以太坊网络交易的 Decentraland 场景之前，你需要确保它没有漏洞或恶意用户可以利用的缺口。

Sepolia 测试网络是以太坊的一个替代版本，专门用于运行测试。

Sepolia 网络中的代币没有真实价值，因此你可以放心犯错，而不会带来任何真实风险。你可以通过使用水龙头免费补充任何丢失的代币：

* Sepolia Ether 水龙头（<https://www.alchemy.com/faucets/ethereum-sepolia/>)

如果你正在开发一个会触发交易的场景，那么在 Sepolia 网络中测试这些交易是免费的，因为你发送的代币没有价值。在主网上，你每进行一次测试交易，至少都需要支付真实的 Ether gas 费用。

一旦你确信代码按预期工作且不会被利用，就可以部署到以太坊主网。

## 区块链重组

有时，多台机器会在大致相同的时间创建新的替代区块。这会造成问题，因为这会把链分叉成两个不断分歧的版本，并且它们可能彼此矛盾。当分叉发生时，以太坊通过始终优先选择最长链并丢弃所有较短的链来解决这一问题。即使两个竞争链可能同时存在，很快其中一条链就会添加另一个区块并超过另一条。由于解决挖矿算法需要时间，竞争链要想完全同步增长会变得越来越困难。迟早会有一条链胜出。

当一条链超过另一条链且争议得到解决时，采用了较短链的机器需要进行调整。这就是所谓的“重组（reorg）”。它们需要回滚其所在分支中所有区块里的交易，直到回到分叉发生的那个点。然后它们需要添加来自更长且被视为合法的分支的新区块。

被回滚的交易可能会重新回到待处理交易池，直到再次被矿工拾取（或者被丢弃）。这些交易支付的任何 gas 费用也会被回滚。

刚刚添加到链尾的区块，基于上面解释的机制，被回滚的概率相当大。随着后续区块不断被添加到链尾，位于区块链更早位置的区块被回滚的可能性会越来越小，因为那需要更大规模的重组。因此，在一笔交易之后，每新增一个加到链尾的区块，都被称为该交易的一次确认。

在创建使用链下信息的应用程序（或场景）时，你应该注意重组的发生。你可能希望只有在达到一定数量的确认后，才将交易视为已验证，并且该交易不再处于链的最末端。

使用多次确认会使信息非常稳定，但交易反映出来的时间会很长。

使用较少的确认时，变化会更快地反映出来，但在发生重组时，有时会出现看起来像是撤销了交易的短暂异常。如果这些交易在你的场景中会带来链下后果，那么你可能也需要以某种方式撤销这些后果。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/qu-kuai-lian-ji-cheng/ethereum-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
