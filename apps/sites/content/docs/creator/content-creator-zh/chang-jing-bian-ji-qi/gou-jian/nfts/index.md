> 向场景添加 NFT 肖像

# NFT

你可以将 NFT（非同质化代币）添加到你的场景中，并以相框形式显示。

OpenSea 支持的所有图片和 GIF 格式也都受 Decentraland 的相框支持。目前不支持视频或音频格式的 NFT。那些也具有 3D 表现形式的 NFT，例如 Decentraland 可穿戴物品，在相框中会以 2D 图像显示。

## 添加 NFT

使用 NFT 智能物品，你可以在 **智能物品** 资源包中找到它，或者直接搜索 *NFT* 在上方的搜索栏中。将 NFT 物品的副本拖到你的场景中并选中后，你可以配置一些字段。

![](/docs/_remote/e6ffb898b183dc0d0a5e93be.webp)

主要需要配置的字段决定要显示哪个 NFT：

* **网络**：你的 NFT 所在的区块链网络。默认使用以太坊主网，但你也可以选择 Polygon（matic），或者多个测试网络之一。
* **合约**：该 NFT 所属系列的合约地址（例如：Cryptokitties、SuperRare、Decentraland 2019 万圣节可穿戴物品等）
* **代币**：这个特定 NFT 的唯一 ID

要获取这些信息，最简单的方法是在 Decentraland Marketplace 中查找它们，然后查看 URL。例如，从下面这个物品的 URL 中：

*<https://market.decentraland.org/contracts/0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0/tokens/20175>*

你可以推断出合约是 *0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0* （指的是 SuperRare），ID 是 *20175*.

同样地，你也可以从该代币的 OpenSea URL 中获取这些信息。例如，从下面这个物品的 URL 中：

*<https://opensea.io/assets/0x31385d3520bced94f77aae104b406994d8f2168c/2614>*

你可以推断出合约是 *0x31385d3520bced94f77aae104b406994d8f2168c* 而 ID 是 *2614*.

NFT 智能物品中还可以配置的其他可选字段有：

* **边框样式**：默认相框样式带有发光边缘，可能与作品或你的场景风格不匹配。还有其他几种可选样式，风格各异，从巴洛克到极简主义，甚至还有在画作角上贴胶带的样式。
* **颜色**：带透明背景的 NFT 会被赋予一种背景色，默认是紫色。你可以选择其他任意颜色。请注意，某些相框样式，例如 *无*，完全不包含背景色。


> [!WARNING]
> **📔 注意**：只有在以预览模式进入场景时，你才能看到这些更改生效。这些更改都不会修改你在编辑模式下拖动的智能物品的表现形式。


参见 [展示 NFT](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/display-a-certified-nft/) ，了解 Decentraland 如何处理 NFT 肖像的更多详情。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/nfts.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
