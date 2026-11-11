# 集合

集合是由……组成的一组 [可穿戴物品](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/) 和 [表情动作](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes/) 在同一离链命名空间或链上合约中定义。

它们本身并不是 [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 但它们的单个项目通常可以使用以下方式从内容服务器获取 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/).

### URN 和指针 <a href="#pointers" id="pointers"></a>

集合使用 URN 来标识，它们是其项目 URN 的前缀。URN 主要有 4 种类型：

* `v1` 具有 Foundation 批准内容的集合（已弃用）。
* `v2` 具有社区批准内容的集合。
* `thirdparty` 其项目与 Decentraland 之外的 NFT 相关联的集合。
* `off-chain` 集合，其中大多包含新头像的默认项目。


> [!NOTE]
> 尽管 `v1` 集合已被弃用，不会再创建或更新，但它们仍然有效，其项目仍可使用。


#### 第 1 和第 2 版集合

旧的 `v1` 和当前的 `v2` 集合命名空间都具有相同形式的 URN：

```
urn:decentraland:<blockchain>:<collections-version>:<contract-address>
```

例如：

```
# 弃用的 v1 空间中的一个集合：
urn:decentraland:mainnet:collections-v1:DCL Test Masks

# v2 社区批准空间中的一个集合：
urn:decentraland:matic:collections-v2:0x25a1d66891d44cdf7b8c45802489c1dea7aadf8b
```

如果 `:<id>` 片段附加在末尾，URN 就会变成一个 [指针](/docs/contributor/contributor-zh/nei-rong/pointers/) 指向集合内某个项目的指针。请参见 [可穿戴物品指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/wearables/#pointers) 和 [表情动作指针](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/emotes/#pointers) 以了解相关信息。

#### 第三方集合

第三方集合，也称为 [关联可穿戴物品](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md)，与常规可穿戴物品不同，因为它们绑定到 Decentraland 之外已存在的 NFT。它们允许玩家在世界内的头像上展示其个人收藏中的 NFT。

它们的 URN 看起来像这样：

```
urn:decentraland:<blockchain>:collections-thirdparty:<third-party-id>:<collection-id>
```

你可以在 [关联可穿戴物品](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md) 面向创作者的文档中找到关于这些内容的大量信息。

### 发现集合

内容服务器可以使用以下方式向你提供所有已知集合的列表： [`/lambdas/collections`](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/getCollections) 端点。

响应将是一个带有 `collections` 属性的对象，其中包含一个 URN 和名称数组。例如：

```json
{
  "collections": [
    {
      "id": "urn:decentraland:amoy:collections-v2:0xff5d4ebc6bc1ff7262cab42d3c693d953f4614d2",
      "name": "Winter clothes collection"
    },
    // ……还有更多
  ]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/nei-rong/collections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
