> LAND 租赁

# 租赁

## 术语表

**土地所有者：** 拥有 LAND 的账户（地址），它可以是一个地块、一个庄园，或两者都有。

**租户：** 从 LAND 所有者那里租用 LAND 的账户（地址）。这也是唯一可以更改拥有操作权限的地址的账户。

**操作权限：** 拥有此权限的地址是唯一可以在该 LAND 上部署场景的地址。

**交易：** 需要支付 gas 的以太坊区块链交易。

## 简介

新的租赁系统允许 LAND 所有者和租户 **以安全且无需信任的方式租赁 LAND** 通过使用存储在由 Decentraland 基金会管理的服务器中的签名（链下）与以太坊交易（链上）的组合来实现。

例如，DJ 可以找到一块很棒的 LAND 地块，把它租下来并部署一个夜总会，每周六进行演出。一所大学可以租下一个庄园并为学生建造校园。

下面你会找到租赁 LAND 所需遵循的所有步骤，以及双方涉及的交易。

## 针对 LAND 所有者

### 将 LAND 出租

作为 LAND 所有者，你可以在以下位置将你的 LAND（地块或庄园）列为可出租： [市场](https://market.decentraland.org/) > 我的资产 > LAND。

为了在链上完成此操作，LAND 所有者必须先批准 Rent 智能合约代表其使用 LAND。然后每个上架都还需要所有者的签名。

![](/docs/_remote/1a34b8c7a8542b15716940f1.webp)

你可以用 MANA 设置每天的租金价格，以及允许他人租用的天数。每天的价格乘以租期中的天数，就是租户需要支付的金额 **，预先支付，且总计** 作为这次租赁的费用。

![](/docs/_remote/d0b5696229c03215afc73455.webp)

在定义每日价格后，你需要选择用户可以租用你的 LAND 的天数。例如，如果你只选择 7 天和 30 天，那么租户只能在这两个选项中选择。如果租户选择了 30 天选项，那么租期将从确认之日起算。

![](/docs/_remote/f0859bad5d58c27a481d636a.webp)

你还可以为上架设置一个到期日期。这意味着，如果 LAND 在所选日期之前没有被租出，则该上架会从 Marketplace 中移除。此外，智能合约会拒绝已过期的签名，以便没有人可以按先前选择的上架价格和时长进行租赁。这是一种安全措施，可防止其以不希望的价格或时长被租出。

![](/docs/_remote/f4c314e8b64de94a0f2045e5.webp)

设置好价格、租期和上架到期日后，你的 LAND 将在 Marketplace 中显示为可租用。


> [!NOTE]
> 💡 当 LAND 被租户租用后，在被收回之前不能出售。潜在买家的出价也无法接收。



> [!NOTE]
> 💡 即使 LAND 被租出，投票权仍由 LAND 所有者保留。


### 编辑或取消上架

在 LAND 被列为 Marketplace 中可租用之后、且在任何人租用之前，你可以通过点击 LAND 详情中的铅笔图标来编辑上架条件。你也可以将该上架从 Marketplace 和区块链中移除。

![](/docs/_remote/89bcf7626763f867ddbc8c00.webp)


> [!NOTE]
> 💡 编辑和取消都需要一笔交易，会消耗 gas。更多详情请参见下方的交易部分。


### 租期结束后

租期结束后，你可以选择 **收回你的 LAND，或再次将其列为可出租**.

**操作权限不会自动转回 LAND 所有者**。为了重新获得它们，LAND 所有者必须发送该交易并支付 gas 费用来收回 LAND。确认交易后，操作权限将从租户处移除并归还给 LAND 所有者。

![](/docs/_remote/f43022686a5bd15b6c02ac8a.webp)

另一种选择是再次将 LAND 列为可出租，而不是将其收回。这不需要再支付一笔交易费用，但是 **在新的租户确认新的租赁之前，操作权限会保留在前一位租户手中。**

LAND 所有者可以为新的上架编辑价格、租期和上架到期日。

![](/docs/_remote/6f6b27950ff42602c5b34fda.webp)

这两个操作都可以在 Marketplace 中的 LAND 详情页面完成。

![](/docs/_remote/2b6877fde55556d4100fd7e0.webp)

### 租赁状态

你可以在 我的资产 > 商店 > 租赁中 查看任何已出租 LAND 的状态。可能的状态有：

* 已列为可出租 - 上架已确认，用户可在 Marketplace 中租用
* 租赁期结束 - 在此阶段，LAND 可由 LAND 所有者选择收回或再次列为可出租
* 租到 *“日期”* - LAND 已经被租出，租户拥有操作权限，直到它被收回或被其他用户重新租用

![](/docs/_remote/486197d9f7fa915ceb7f0183.webp)

## 针对租户

### 租用 LAND

所有用户都可以在 Marketplace 的 LAND 部分找到可租用的 LAND。

![](/docs/_remote/95cd665129b4cbe803dc76df.webp)

有些 LAND 可以出售或出租。如果两种选项都可用，你可以通过点击“出售/出租”切换按钮查看每种选项的可用条件。

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

一旦你找到想要租用的 LAND，就需要选择租期，也就是你将拥有该 LAND 的天数。选择租期后，你会看到租赁需要支付的总价格。

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

在继续之前，你需要先批准 Rent 智能合约从你的账户中提取 MANA。

在确认租赁之前，你可以决定由谁管理该 LAND（操作权限）。可以是你自己，也可以是你选择的任何其他地址。

![](/docs/_remote/2d83c8c5fc075dc07df8532c.webp)

操作权限之后可以由租户（最初租用该 LAND 的地址）更改，方法是从 [Builder](https://builder.decentraland.org/).

![](/docs/_remote/506dbf3015f1fd2ea5698b48.webp)

选择完所有细节并批准 Rent 智能合约处理你的 MANA 后，你可以通过发送一笔交易来确认租赁。

这样就完成了！你可以开始在你的 LAND 上工作，并使用 Builder 或 SDK 部署一个场景。

![](/docs/_remote/76317d506da755beb44eeaf7.webp)

注意：在租期结束后，除非 LAND 所有者将其收回，或其他人重新租用，否则租户仍将拥有操作权限。 **确保你在租期结束前保存好内容，否则内容可能会丢失。**


> [!NOTE]
> 💡 租赁 LAND 并不会将投票权转移给租户。投票权仍由 DAO 在此处定义的 LAND 所有者保留 [提案](https://governance.decentraland.org/proposal/?id=c98bd010-74b1-11ed-a9bf-f772a12a0556)


## 交易

为了 **安全性和去中心化**，租赁系统依赖以太坊区块链作为真实来源。

但是，并非所有相关操作都需要在区块链上记录。如果那样做，对双方来说成本都会过高。

区块链上的交易应尽量最少，以便提供一个 **强大且无需信任的 LAND 租赁系统，同时保持其可负担性。** 以下是所有需要考虑的交易：

### 针对土地所有者

#### 列为可出租

在首次将第一个地块或庄园列为可出租之前，LAND 所有者需要允许 Rents 智能合约代表其操作 LAND。对于地块只需执行一次，对于庄园也只需执行一次。

![](/docs/_remote/24e324959f412a5ef38535ed.webp)

#### 收回 LAND 或再次列为可出租

在租赁期结束后， **操作权限不会自动转回 LAND 所有者**。为了重新获得它们，LAND 所有者必须发送该交易并支付 gas 费用来收回 LAND。

![](/docs/_remote/9b7b0037cff4903f1d9e7022.webp)

另一种选择是再次将 LAND 列为可出租，而不是将其收回。这不需要支付另一笔交易费用，但在新的租户确认新的租赁之前，操作权限会保留在前一位租户手中。

#### 编辑上架

如果价格、租期或到期日期中的任一项发生变化，LAND 所有者必须发送一笔交易，以防止有人直接在智能合约上使用之前的上架签名（而不是通过 Marketplace 界面），并以更低的价格或不希望的时长获得它。

![](/docs/_remote/985e0faa31bf88ac207e0846.webp)

### 针对租户

#### 允许 Rent 合约操作你的 MANA

无论是地块还是庄园，任何想要租用 LAND 的用户都必须发送一笔交易，允许 Rent 智能合约代表其操作 MANA。这是必需的，因为当租赁生效时，智能合约必须提取 MANA 并将其转给 LAND 所有者。从那时起，这一步只需对所有要租赁的 LAND 执行一次。

#### 租用 LAND

在批准 Rent 智能合约操作你的 MANA 后，你就可以确认你的第一次租赁了。一旦你找到想要的 LAND，选择租期，并确认租赁交易，操作权限就会转移到所选地址。

如果你想租用另一个地块或庄园，只需发送一笔交易来确认即可，无需再次批准智能合约操作你的 MANA。

![](/docs/_remote/34fb306b490b37363a4d6562.webp)

#### 更改操作人

在租用 LAND 时，用户可以选择哪个地址将拥有该 LAND 的操作权限。如果想要更改该地址，就必须发送一笔交易。

![](/docs/_remote/e38a6d9159fa451ba7b2570b.webp)

## 智能合约钱包

租赁功能在很大程度上依赖链下签名。链下操作允许土地所有者在不支付交易成本的情况下将 LAND 列为可出租。

通过签署上架，Rent 智能合约可以验证该上架是由签名者创建的。

签名的特点在于它需要私钥。所有 EOA（外部拥有账户）都拥有私钥，并且可以用它来签署上架。随后，Rentals 智能合约会在执行租赁时验证 EOA 生成的签名。

智能合约钱包，也就是智能合约，没有私钥，因此无法签署消息。取而代之的是，必须由智能合约钱包授权的 EOA 来签名。

为了支持这些签名，Rent 智能合约会按照以下标准，通过智能合约钱包验证签名是否有效： [EIP-1271](https://eips.ethereum.org/EIPS/eip-1271) 标准。如果签名有效，就可以执行租赁。

智能合约钱包不仅必须具备 EIP-1271 中定义的标准签名验证方法，还必须具备以下标准中定义的代币接收方法： [EIP-721 标准](https://eips.ethereum.org/EIPS/eip-721)。这是收回 LAND 时所必需的，因为 Rent 智能合约会调用 `safeTransferFrom` 将 NFT 返还给智能合约钱包，如果它没有实现适当的 `onERC721Received` 函数，它就无法成功找回 LAND。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/shi-chang/rentals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
