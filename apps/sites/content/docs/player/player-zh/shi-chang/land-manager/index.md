> 管理 LAND 和 Estate 代币

# LAND 管理器

Land 管理器可让你管理你的 LAND 和 Estate 资产。

访问 Land 管理器： <https://builder.decentraland.org/land>.

Land 管理器可让你：

* **命名** 为你的地块和 Estate 命名，并为它们添加公开描述。
* **合并** 将 LAND 地块合并为一个 Estate。
* **解散** 将一个 Estate 解散为单独的 LAND 地块。
* **转移** 将你的地块和 Estate 转移给另一位用户。
* **授予权限** 给其他用户，以便他们编辑你拥有的地块。

## 管理你的 LAND

要查看你的 LAND 代币，请点击 **管理你的 LAND**。在这里你会找到你所有地块和 Estates 的列表，包括你已挂牌出售的任何地块。

点击下方列出的某个地块或 Estate： **土地**，你可以编辑其名称、描述，设置操作员，或直接将其转移到另一个钱包地址。

![](/docs/_remote/2442e30de2eeb3bfbb62d341.webp)

## 创建 Estate

LAND Estate 可让你将两个或多个直接相邻的 LAND 地块关联起来，从而更便于管理更大的 LAND 持有量。Estate 尤其适合用于构建跨越多个地块的大型场景。

Estate 中的地块必须直接相邻，且不能被道路、广场或任何其他地块隔开。

要创建你的第一个 Estate，你需要拥有两个或更多相邻的 LAND 地块。

1. 打开 **我的 LAND** 并选择你想添加到 Estate 中的其中一个地块。
2. 在该地块的详情页中，点击 **创建 Estate**.
3. 系统会向你展示以你所选地块为中心的 Atlas 视图，并高亮显示你拥有的其余相邻地块。选择你想包含在 Estate 中的不同地块。 ![](/docs/_remote/4ef95117e0b2e23f246636d8.webp) ![](/docs/_remote/5d4f1b545b9d7baf0d140f45.webp)
4. 点击 **继续**.
5. 为你的 Estate 输入名称和描述。这些信息将公开显示在 Atlas 中，就像任何单独地块的名称和描述一样。

   ![](/docs/_remote/75a307fdd4e3535bf406e003.webp)
6. 在你的以太坊客户端中确认此交易，并等待网络验证。

创建第一个 Estate 后，你会看到一个名为 Estates 的新标签页。在此页面中，你可以查看和管理你所有的 Estates。

当你创建一个新的 Estate 时，实际上就是将你的地块转移到一个新的代币中。这些 Estates 由 ERC721 代币表示（与其他 NFT 一样）。你将不再在 *我的 LAND*下看到单独的地块，它们也不会出现在 MetaMask、Mist、Trezor 或 Ledger 钱包中，也不会在 Etherscan 上以你的地址显示。

## 编辑地块或 Estate

你可以编辑你拥有的任何地块或 Estate 的名称和描述。这些信息将公开显示在 Atlas 中。

要编辑地块或 Estate：

1. 前往你想编辑的地块或 Estate 的详情页，并点击 **编辑**.

   ![](/docs/_remote/3dc2640de68eb6f3a61f97b6.webp)
2. 点击 **提交**.
3. 在你的以太坊客户端中确认此交易，并等待网络验证。

## 授予权限

你可以授予其他用户权限来编辑某个地块或 Estate 中的内容。这样该用户就可以向场景部署代码，但无法出售该代币。

被授予权限的用户也可以在 Marketplace 中更改名称或描述。

要为你的 LAND 授予权限：

1. 前往该地块或 Estate 的详情页，点击三个点，然后点击 **设置操作员**. ![](/docs/_remote/1ebd7dd1f5d8b72647811f3b.webp)
2. 在表单中填写所需信息。

   ![](/docs/_remote/8de80dd1532ffea8a620d5b2.webp)
3. 点击 **提交**.
4. 在你的以太坊客户端中确认此交易，并等待网络验证。

## 查看你的活动历史

点击屏幕顶部的铃铛图标，打开通知页面。

![](/docs/_remote/350d35176446e1c73654bfae.webp)

通知页面会显示你最近执行的所有交易列表及其状态。

点击某笔交易，即可在 Etherscan 上查看其更多详情。

## 转移 LAND

要将 LAND 地块或庄园转移给另一位用户：

1. 前往你想转移的地块或 Estate 的详情页，并点击 **转移**.

   ![](/docs/_remote/d4abb6744a7112af5514a3f1.webp)
2. 输入收件人以太坊钱包的公开地址。


> [!WARNING]
> **📔 注意**: 请仔细核对该地址，因为此操作无法取消。虽然收件人之后总能把 LAND 转回给你，但原始所有者无法撤销该操作。


![](/docs/_remote/186df165a7bc48578054cfb9.webp)

3. 点击 **提交**.
4. 在你的以太坊客户端中确认此交易，并等待网络验证。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/shi-chang/land-manager.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
