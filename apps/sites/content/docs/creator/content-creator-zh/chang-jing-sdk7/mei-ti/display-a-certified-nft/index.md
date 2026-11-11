> 了解如何在你的场景中显示你拥有的已认证 NFT

# 显示已认证的 NFT

你可以在你的 Decentraland 场景中显示一个你拥有的 2D NFT（非同质化代币）。

NFT 的图像和其他数据会根据代币的合约和 ID 从 API 中获取。任何在 [OpenSea](https://opensea.io/) 也可以在 Decentraland 的 NFT 相框中显示。


> [!NOTE]
> **💡 提示**：在 [Creator Hub 中的场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)，你可以使用一个 **NFT** [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 用于以无代码方式实现这一点。


相框会根据 NFT 图像的尺寸进行调整显示。如果图像尺寸为 512 X 512 像素，相框会保持原始大小。如果图像具有不同的尺寸，相框将被调整大小并拉伸以匹配这些尺寸。


> [!NOTE]
> **💡 提示**: 如果你想对默认生成的图像进行拉伸或调整大小，你可以更改 `缩放` 实体的 `Transform` 组件，则抛出错误。


## 添加 NFT

添加一个 `NftShape` 组件到一个实体上，以在你的场景中显示一个 2D 代币。

```ts
const nft = engine.addEntity()

Transform.create(nft, {
	position: Vector3.create(8, 1, 8),
})

NftShape.create(nft, {
	urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
})
```

该 `NftShape` 组件必须通过一个包含以下内容的参数进行实例化：

* `urn` 字段。该字段接受一个应遵循以下结构的字符串：

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

该字符串包括：

* 该 *网络* 该代币存在于其中的网络。请参见下面支持值列表。
* 该 *合约标准* 该代币所基于的，例如 `erc721`
* 该 *合约* 该代币的（例如 CryptoKitties 合约）
* 该 *id* 要显示的特定代币的

例如：

`urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536`

上面的示例会获取一个 NFT，其合约地址为 `0x06012c8cf97BEaD5deAe237070F9587f8E7A266d`，以及特定标识符 `558536`。对应的资产可在 OpenSea 上找到： [https://opensea.io/assets/0x06012c8cf97bead5deae237070f9587f8e7a266d/558536](https://opensea.io/assets/0x06012c8cf97BEaD5deAe237070F9587f8E7A266d/558536).

以下值受支持，适用于 *网络*:

* `ethereum`：Ethereum 主网
* `matic`：Polygon Matic
* `klaytn`：Klaytn
* `bsc`：BNB Chain
* `arbitrum`：Arbitrum
* `arbitrum_nova`：Arbitrum Nova
* `avalanche`：Avalanche
* `optimism`：Optimism
* `solana`：Solana
* `base`：Base
* `blast`：Blast
* `zora`：Zora


> [!WARNING]
> **📔 注意**：图像大小不得超过 **6 MB**。如果图像更大，它将不会在场景中渲染。


## 自定义相框

默认情况下，图像会带有紫色背景，并在周围显示一个带有脉动发光材质的相框。你可以设置以下属性，使其更好地匹配 NFT 和场景的风格：

* `color`：决定模型的背面，以及在 NFT 图像具有透明度时图像的背景。
* `样式`：从几个预设选项的枚举中选择一个相框模型。请使用该枚举中的一个值 `NftFrameType`，其中包含所有可用样式的列表。

```ts
NftShape.create(nftEntity, {
  urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
  color: Color3.Green(),
  style: NftFrameType.NFT_GOLD_EDGES,
})
```

![](/docs/_remote/8698f94b4eeaa8c4125b5ae6.webp)

以下是所有受支持的相框样式完整列表：

* `NFT_CLASSIC`
* `NFT_BAROQUE_ORNAMENT`
* `NFT_DIAMOND_ORNAMENT`
* `NFT_MINIMAL_WIDE`
* `NFT_MINIMAL_GREY`
* `NFT_BLOCKY`
* `NFT_GOLD_EDGES`
* `NFT_GOLD_CARVED`
* `NFT_GOLD_WIDE`
* `NFT_GOLD_ROUNDED`
* `NFT_METAL_MEDIUM`
* `NFT_METAL_WIDE`
* `NFT_METAL_SLIM`
* `NFT_METAL_ROUNDED`
* `NFT_PINS`
* `NFT_MINIMAL_BLACK`
* `NFT_MINIMAL_WHITE`
* `NFT_TAPE`
* `NFT_WOOD_SLIM`
* `NFT_WOOD_WIDE`
* `NFT_WOOD_TWIGS`
* `NFT_CANVAS`
* `NFT_NONE`

某些相框比其他相框使用更多材质。例如，默认相框会为 NFT 本身添加 1 个材质、为有颜色的背景平面添加 1 个材质，并为相框添加 2 个材质（与同样式的其他相框共享）。如果你需要减少材质数量，以 [保持在场景限制内](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)，请选择更简单的样式。例如，“none”仅为 NFT 本身使用 1 个材质。


> [!NOTE]
> **💡 提示**：使用 Visual Studio Code（或其他 IDE），通过输入 `NftFrameType.` 并等待智能建议显示选项列表。使用 `NftFrameType.NFT_NONE`即可按原样显示纯净的 NFT，不带相框或背景色。


## 打开 NFT 界面

打开一个预先构建的界面，显示 NFT 的名称、所有者和描述。它还包括 NFT 的当前价格和最近一次成交价格（如适用），以及一个链接到 OpenSea 上该 NFT 页面 的按钮，在那里可以查看更多信息并购买该 NFT。

![](/docs/_remote/2e438b55141b8a0db1d92f61.webp)

通过调用函数打开此界面 `openNftDialog()`。此函数需要一个对象作为参数，其中包含一个单独的 `urn` 字段。该字段接受一个应遵循以下结构的字符串：

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

例如：

`urn:decentraland:ethereum:erc721:0x00...000:123`


> [!WARNING]
> **📔 注意**：必须通过按钮事件来打开该 UI，以防止恶意刷屏。该按钮事件不一定需要在同一个相框上，也不一定需要在一个 `NftShape`.


要通过点击操作打开此界面，请添加以下内容：

```ts
import { openNftDialog } from '~system/RestrictedActions'

// 添加指针碰撞器，以便可以点击它
MeshCollider.setBox(myEntity, ColliderLayer.CL_POINTER)

// 添加 onPointerDown 回调事件。
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: '点击' },
	},
	function () {
		openNftDialog({
			urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536',
		})
	}
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/mei-ti/display-a-certified-nft.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
