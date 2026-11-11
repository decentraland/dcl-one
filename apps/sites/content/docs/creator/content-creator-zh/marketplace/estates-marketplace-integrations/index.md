> 如果你想在自己的市场中集成 Decentraland 的庄园，需注意的重要事项

# 庄园市场集成

## 将 Decentraland 的 Estate 集成到你的市场中

Decentraland 的 Estate 是符合 ERC721 标准的 NFT，并运行在以太坊主网。因此，很多第三方市场都可以交易它们。为此，该市场必须遵循某些规则，以确保用户交易的安全。

如你所知，每个 [Estate](https://docs.decentraland.org/decentraland/faq/#what-is-an-estate) 都是一组 [LANDs](https://docs.decentraland.org/decentraland/faq/#what-is-land) ，并且它有一个 *bytes* 哈希与该组关联，它被称为 *`指纹`*。每当 Estate 因添加或移除一个 LAND 而发生变化时，它的 *`指纹`* 也会改变。

对于市场，尤其是没有托管系统的市场，强烈建议记录 Estate 的 *`指纹`* ，当有人将其挂牌出售或提出报价时。这样，当订单/报价成功执行时，当前所有者就不能通过试图抢先执行订单来更改该 estate。

不建议挂牌空的 Estates。Estate 智能合约有一个方法 `getEstateSize` ，它返回 Estate 中 LAND 的数量。如果结果是 `0`，我们建议不要挂牌该 Estate。如果你仍然想挂牌，你会看到如下图像：

![](/docs/_remote/3ea9308d2dc5bf30fe069b53.webp)

## 示例

### 未使用 Estate 指纹

* Bob 拥有 Estate1，包含 LAND (1,1) 和 (1,2)。Estate1 指纹：hash1
* Bob 向 Estate1 添加了 LAND (1,3)。Estate1 现在包含 LAND：(1,1)、(1,2) 和 (1,3)。Estate1 指纹：hash2（指纹已更改）
* Bob 从 Estate1 中移除了 LAND (1,1)。Estate1 现在包含 LAND：(1,2) 和 (1,3)。Estate1 指纹：hash3（指纹已更改）
* Bob 将 Estate1 挂牌出售。该列表在以太坊主网上链创建，包含 Estate 智能合约地址、Estate ID、价格和到期日期。
* Alice 发送了一笔交易来购买 Estate，并指定要购买的 estate id 和预期支付的价格。
* Bob 检测到有人正试图购买他的 Estate1，并发送了一笔比 Alice 更高 gas 费的交易，以从 Estate1 中移除 LAND (1,2) 和 (1,3)。
* Bob 的交易先被打包。Estate1 现在有 0 个 LAND。Estate1 指纹：hash4（指纹已更改）
* Alice 的交易随后被打包。Alice 购买了一个上面没有任何 LAND 的 Estate1。这意味着 Alice 被 Bob 抢先交易了（并被盗/被骗）。

### 使用 Estate 指纹

* Bob 拥有 Estate1，包含 LAND (1,1) 和 (1,2)。Estate1 指纹：hash1
* Bob 向 Estate1 添加了 LAND (1,3)。Estate1 现在包含 LAND：(1,1)、(1,2) 和 (1,3)。Estate1 指纹：hash2（指纹已更改）
* Bob 从 Estate1 中移除了 LAND (1,1)。Estate1 现在包含 LAND：(1,2) 和 (1,3)。Estate1 指纹：hash3（指纹已更改）
* Bob 将 Estate1 挂牌出售。该列表在以太坊主网上链创建，包含 Estate 智能合约地址、Estate ID、价格和到期日期。
* Alice 发送了一笔交易来购买 Estate，并指定 estate id、预期支付的价格，以及 **她看到的指纹（hash3）**.
* Bob 检测到有人正试图购买他的 Estate1，并发送了一笔比 Alice 更高 gas 费的交易，以从 Estate1 中移除 LAND (1,2) 和 (1,3)。
* Bob 的交易先被打包。Estate1 现在有 0 个 LAND。Estate1 指纹：hash4（指纹已更改）
* Alice 的交易被回滚，因为智能合约检查到 Alice 发送的参数中的指纹与当前 Estate1 的指纹不匹配（hash3 != hash4）。这一检查阻止了 Alice 购买一个非目标的 Estate。

## Estate 智能合约指纹接口

Estate 的智能合约符合一个 [指纹接口](https://github.com/decentraland/land/blob/master/contracts/estate/EstateStorage.sol#L19)。当 Estate 被挂牌出售或收到报价时，通过调用 *`getFingerprintV2(uint256 estateId)`* 并存储它返回的值。要在执行时检查该订单/报价是否仍然有效，请调用 *`verifyFingerprint(uint256 estateId, bytes fingerprint)`* 并使用存储的值。

始终存储 *`getFingerprintV2`*&#x8FD4;回的值。旧的 *`getFingerprint`* 方法仅为向后兼容而保留，正在逐步弃用：从 2026 年 11 月 26 日开始， *`verifyFingerprint`* 将不再接受它生成的值。新的集成应使用 *`getFingerprintV2`*.

你可以查看一个可运行的生产示例 [此处](https://github.com/decentraland/marketplace-contracts/blob/master/contracts/marketplace/MarketplaceV2.sol#L382).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/marketplace/estates-marketplace-integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
