> 第三方代币的可穿戴表现形式

# 关联可穿戴物

## 关于

根据 [Linked Wearables 的最初 DAO 提案](https://governance.decentraland.org/proposal/?id=14e76cc0-2bc7-11ec-ac84-77607720a240) （此前称为：Third Party Wearables）， [包含最终定义的草案提案](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231) 和 [Linked Wearables 重设计提案](https://decentraland.org/governance/proposal/?id=65caf8d1-8601-49a5-ae11-b0b99d7fdd3c)，本文档将作为文档资料，涵盖 Linked Wearables 功能的所有相关细节。

本文档主要面向那些希望让其用户能够在漫步 Decentraland 时将其 NFT 作为穿戴物展示的 NFT 社区代表。

## 什么是 Linked Wearables？

Linked Wearables 是源自 Decentraland 之外的 NFT 的 3D 表示，可在世界中作为穿戴物使用，可装备到头像上，并可在背包中找到。它们不是 [常规穿戴物](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/README.md#what-are-wearables)。它们看起来相同，并遵循常规穿戴物 [指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/) ，但承载着完全不同的含义。

Linked Wearables 不存在于传统穿戴物集合中（它们属于一种特殊类型的集合），没有稀有度，也不能在 [一级](https://market.decentraland.org/browse?assetType=item&section=wearables) 或 [二级](https://market.decentraland.org/browse?assetType=nft&section=wearables&vendor=decentraland&page=1&sortBy=recently_listed&onlyOnSale=true&viewAsGuest=false&onlySmart=false) 市场上出售。它们只是 **与外部 NFT 关联的世界内表示。**

> 想象你有一个名为 ‘Cryptojackets’ 的 NFT 项目，其中每个 NFT 都是一种不同的 2D 夹克，而你希望用户能在他们的 Decentraland 背包中拥有夹克的 3D 表示。Linked Wearables 让你可以将 NFT 的 3D 表示作为 Decentraland 中的穿戴物提交。无需铸造新的代币，而你的现有 NFT 项目将拥有一个全新的开箱即用功能可供提供！

所有 Linked Wearables 都定义在一个 **Linked Wearable Collection**中。我们将在本文后文看到如何创建一个。

## Linked Wearables 如何表示 NFT？

穿戴物通过在 [Builder 网站](https://decentraland.org/builder) 中创建 Linked Wearable Collection，并在创建穿戴物时设置 NFT 的表示方式，从而与 NFT 关联。

我们支持 4 种将你的穿戴物与 NFT 关联的机制。所有这些机制都使用 NFT 的 token id 来匹配它们。

下表展示了上述机制：

| 匹配     |                                                                                                                                     |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| 所有 NFT | 拥有该集合任一 NFT 的用户将拥有该穿戴物                                                                                                              |
| 单个 NFT | 拥有由 TOKEN ID 指定的 NFT 的用户将拥有该穿戴物。 *例如 123456。若用户拥有 TOKEN ID 为 123456 的 NFT，则将拥有该穿戴物*                                                 |
| 多个 NFT | 拥有由多个 TOKEN ID 指定的任一 NFT 的用户将拥有该穿戴物，这些 ID 以逗号分隔。 *例如 123456, 123457, 123458。若用户拥有 TOKEN ID 为 123456、123457 或 123458 的 NFT，则将拥有该穿戴物* |
| NFT 范围 | 拥有 TOKEN ID 范围内指定的任一 NFT 的用户将拥有该穿戴物。 *例如 \[1, 1000]。若用户拥有 TOKEN ID 为 1、2、3、4、5……或 100 的 NFT，则将拥有该穿戴物*                               |

## Linked Wearables 集合的类型

通常有两种类型的 NFT 集合：

* **手工制作：** 每个代币资产都是量身定制或手工制作的，没有任何自动化流程。
* **程序化：** 每个代币资产并非单独手工制作，而是通过代码自动生成，许多时候基于先前设计和建模的特征。例如： [CryptoPunks](https://opensea.io/collection/cryptopunks) 和 [BAYC](https://opensea.io/collection/boredapeyachtclub) 是以程序化方式创建的 2D 头像 NFT 集合的例子。

在 Decentraland 中，我们在 Linked Wearables 集合上也遵循相同原则：手工制作的集合可以是标准 Linked Wearables 集合，自动生成的集合可以是程序化 Linked Wearables 集合。这两种集合在发布费用方面有所不同。请查看 [费用部分](#costs) 以了解更多关于发布费用的信息。

## 创建 Linked Wearables

## 创建 Linked Wearable Collection

创建 Linked Wearable 集合是开始创建我们的 Linked Wearables 的第一步。

Linked Wearables 被分组在集合中，这些集合可由其所有者创建、编辑和删除。每个集合可以包含任意数量的 Linked Wearables。每个集合都将链接到一个 NFT 集合，因此， **需要一个 NFT 合约（兼容 ERC721 或 ERC1155）来创建 Linked Wearable Collection。**

要创建一个新的 Linked Wearable，请按以下步骤操作：

1. 在集合部分，点击 **Create Collection** 按钮。

   ![](/docs/_remote/4e2aa2f187322d76f74c1b4c.webp)
2. 通过点击 Linked Collections 部分下的按钮，选择 Linked Wearable Collection 选项。 **Create Collection** 按钮。

   ![](/docs/_remote/23dbdab92c38a595e6d609ae.webp)
3. 为集合选择一个名称，并通过设置其合约地址以及所在网络，将集合链接到你的 NFT 集合。 **系统会验证该合约，以确保其符合 NFT 合约标准。**

   ![](/docs/_remote/8365f405886986e6be3328d6.webp)
4. 点击 **Create** 按钮以创建该集合。

## 逐个向 Linked Wearables 集合添加穿戴物

与标准穿戴物一样，你也可以逐个上传穿戴物的 3D 模型。

为此，请按以下步骤操作：

1. 点击 **新项目** 按钮。

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. 选择 **单个项目** 选项。

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. 按照 [创建穿戴物指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/) 中所述的步骤上传并配置你的穿戴物，并设置它将如何与你的 NFT 关联。请查看“[Linked Wearables 如何表示 NFT？](#how-do-linked-wearables-represent-nfts)”部分以了解有关如何配置它的更多信息。

   ![](/docs/_remote/3f230f70eb074d1a7d79fecf.webp)

## 批量向 Linked Wearables 集合添加穿戴物

由于 Linked Wearable 集合可以包含大量项目，因此可以批量上传 3D 模型和穿戴物信息。此过程包括为每个项目创建一个包含该项目所需全部资源的 zip 文件。 **仅建议在程序化集合中批量上传穿戴物**.

### 构建穿戴物 ZIP

每个项目都需要构建一个 ZIP 文件，其中包括以下资源：

* 该 **必需的** 穿戴物 3D 模型文件（GLB、GLTF、纹理文件等）。
* 一个 **必需的** `wearable.json` 文件，其中包含穿戴物的信息。
* 一个 **可选的** `thumbnail.png` 文件，其中包含将在 Builder 和世界中显示的穿戴物缩略图。如果未提供，将使用 3D 模型自动生成一个。

3D 模型和可选的 `thumbnail.png` 遵循 [自定义缩略图部分](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-wearables/#custom-thumbnails) 中“上传穿戴物”文章里关于如何创建自定义缩略图的说明。

该 `wearable.json` 与穿戴物内容相关的文件格式如下（按 TypeScript 的类型写法）：

```typescript
type WearableConfiguration = {
  /** 穿戴物的 URN */
  id?: string
  /** 穿戴物名称 */
  name: string
  /** 穿戴物描述 */
  description?: string
  data: {
    /** 穿上该穿戴物时要替换的穿戴物 */
    replaces: WearableCategory[]
    /** 穿上该穿戴物时要隐藏的穿戴物 */
    hides: WearableCategory[]
    /** 用于标识该穿戴物的标签 */
    tags: string[]
    /** 穿戴物的表示形式 */
    representations: WearableRepresentation[]
    /** 穿戴物的类别 */
    category: WearableCategory
  },
  /** 穿戴物与 NFT 的映射定义 */
  mapping: Mapping
}

type WearableRepresentation = {
  /** 表示形式的身体形状 */
  bodyShapes: BodyShape[];
  /** 表示形式主文件的文件路径（GLB、GLTF 等） */
  mainFile: string;
  /** 属于该表示形式的文件路径列表 */
  contents: string[];
  /** 装备此表示形式时要隐藏的穿戴物 */
  overrideHides: WearableCategory[];
  /** 装备此表示形式时要替换的穿戴物 */
  overrideReplaces: WearableCategory[];
}

enum WearableCategory = {
  EYEBROWS = 'eyebrows',
  EYES = 'eyes',
  FACIAL_HAIR = 'facial_hair',
  HAIR = 'hair',
  HEAD = 'head',
  BODY_SHAPE = 'body_shape',
  MOUTH = 'mouth',
  UPPER_BODY = 'upper_body',
  LOWER_BODY = 'lower_body',
  FEET = 'feet',
  EARRING = 'earring',
  EYEWEAR = 'eyewear',
  HAT = 'hat',
  HELMET = 'helmet',
  MASK = 'mask',
  TIARA = 'tiara',
  TOP_HEAD = 'top_head',
  SKIN = 'skin'
}

enum WearableBodyShape = {
  MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale',
  FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
}

type Mapping = SingleMapping | AnyMapping | RangeMapping | MultipleMapping

enum MappingType = {
  SINGLE = 'single',
  ANY = 'any',
  MULTIPLE = 'multiple',
  RANGE = 'range'
}

type SingleMapping = {
  type: MappingType.SINGLE
  id: string
}

type AnyMapping = {
  type: MappingType.ANY
}

type RangeMapping = {
  type: MappingType.RANGE
  from: string
  to: string
}

type MultipleMapping = {
  type: MappingType.MULTIPLE
  ids: string[]
}
```

以下是一个 `wearable.json` 包含每种身体形状对应模型的文件示例：

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "特殊帽子",
  "description": "穿戴物的描述",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat",
    "mapping": {
      "type": "single",
      "id": "123"
    }
  }
}
```

该文件将与 `aMaleModelFile.glb`, `aTextureFile.png`, `aFemaleModelFile.glb` 和 `anotherTextureFile.png`.

要为穿戴物添加自定义缩略图，你可以添加一个 `thumbnail.png` 文件。

关于 `wearable.json` 文件，有几点需要考虑：

* 关于穿戴物类别以及如何选择的所有信息都可以在 [创建穿戴物指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/).
* representations 数组将包含每种身体形状的外观信息。每个穿戴物必须至少包含一个表示形式（它可以包含一个或两个），也就是说，考虑到我们当前拥有的身体形状， `urn:decentraland:off-chain:base-avatars:BaseMale` 或 `urn:decentraland:off-chain:base-avatars:BaseFemale`。每个表示形式将描述每种身体形状将使用哪些模型。
* 映射对象必须配置为将你的 NFT 链接到你的穿戴物的可用机制之一，遵循“[Linked Wearables 如何表示 NFT？](#how-do-linked-wearables-represent-nfts)”部分。

**为项目设置自定义 ID 或 URN**

该 `id` 字段是可选的，可用于创建一个具有特定 ID 的穿戴物，以便将来在批量处理中更新（这在 [批量编辑穿戴物](#editing-wearables-in-bulk) 部分中有进一步说明）。

如果使用 `id` 字段，它必须包含穿戴物的完整 ID。穿戴物的 ID（或 URN）写作 `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id:item-id`。其中， `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id` 可以从集合页面获取，而 `item-id` 是你想使用的项目自定义标识符。

ID 或 URN 遵循特定格式，它们接受：

* 小写字母字符，从 `a` 到 `z`.
* 。所有数字。
* 它们不能包含其他类型的字符或空白。我们建议你将空白替换为 `-`

你可以按照以下步骤，从集合页面获取 ID（或 URN）：

1. 进入你想复制 URN 的集合视图，并在选项菜单中点击 Edit URN 选项：

![](/docs/_remote/72142577aacca52f35921c2a.webp)

2. 复制文本字段下方的标识符：

![](/docs/_remote/8d7643cea8e5a23b82de3fbb.webp)

例如，如果从 UI 中获取到的该集合 URN 或 ID 是 `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection` 并且你是按数字识别你的穿戴物，那么该示例的 URN 将是 `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1`，其中 `1` 是穿戴物的编号。

### 上传流程

当所有文件准备就绪后，要批量上传穿戴物，请按以下步骤操作：

1. 点击 **新项目** 按钮。

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. 选择 **多个项目** 选项。

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. 点击 **浏览你的电脑** 链接以打开文件管理器并选择包含你穿戴物的所有 zip 文件。

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. 检查所有文件是否正确，或者是否需要修复。在这种情况下，要么是穿戴物的模型未设置，要么 `wearable.json` 文件的表示形式设置不正确。

   ![](/docs/_remote/9af2b80ef49b3ab34f730589.webp)
5. 通过点击 **添加更多** 按钮并重新上传同名失败文件，或者使用模态框右上角的垃圾桶图标忽略错误，从而修复任何错误。

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. 通过点击 **上传项目**.

   ![](/docs/_remote/56c36770612421c06d0c4eb5.webp)
7. 进行所有穿戴物的上传。请耐心等待，这可能需要一些时间！

   ![](/docs/_remote/2418ecc5ff115dc5b7837888.webp)
8. 成功！你的项目现在已可在你的集合中使用。

   ![](/docs/_remote/e983b87c56423a0df39dd0f5.webp)
9. 选择你的集合是程序化集合还是标准集合。请查看 [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) 部分，以正确设置你正在为哪种集合类型创建项目。

   ![](/docs/_remote/e70222ca6d6afc784bbdbab3.webp)

### 上传批量项目时的常见错误

* 该 `id` 字段设置为一个已被另一个穿戴物使用的值。
* 该 `id` 字段设置为一个无效 ID 的值。例如，第三方 ID 或集合 ID 属于另一个第三方或集合。
* 没有 `wearable.json` 文件在 zip 中。
* ZIP 文件的根目录中没有 `wearable.json` 文件。
* 该 `wearable.json` 格式或值不正确。
* 文件大于 3MB。Linked Wearables 在大小方面与常规穿戴物有相同限制。
* 自定义可选缩略图图片不是 png 文件。

## 在 Decentraland 中查看穿戴物

Linked Wearables 可以在世界中查看，以检查发布并批准后模型的效果。

要在世界中查看穿戴物，请按以下步骤操作：

1. 点击你想在世界中查看的项目右侧的肉丸菜单（三个水平点）。下拉菜单将出现。选择 **在 Decentraland 中查看**.

   ![](/docs/_remote/c816f508064933bb206516a8.webp)
2. Decentraland 世界将打开。前往你的背包查看该穿戴物。

   ![](/docs/_remote/e27530d66df008ecb1568921.webp)


> [!WARNING]
> **⚠️ 注意**：世界内预览适用于 Decentraland 客户端的过时网页版。暂时还无法在 Decentraland Desktop Client 2.0 中测试它们。


## 编辑 Linked Wearables

## 编辑集合名称

集合的创建者可以重命名集合 **，前提是该集合没有已发布的穿戴物**.

要编辑 Linked Wearable Collection 的名称，请按以下步骤操作：

1. 点击集合名称。

   ![](/docs/_remote/39383f19e2bc56803e0a516b.webp)
2. 为集合选择一个新名称，然后点击保存按钮。

   ![](/docs/_remote/30585b27d6df4a4b988c3e13.webp)

## 删除集合

集合的所有者可以删除集合 **，前提是该集合没有已发布的穿戴物**.

要删除 Linked Wearable Collection，请按以下步骤操作：

1. 点击按钮组最右侧的肉丸菜单（三个水平点）。下拉菜单将出现。选择 **删除**.

   ![](/docs/_remote/72142577aacca52f35921c2a.webp)
2. 确认模态框将出现，如果你想继续，点击 **确定**，否则点击 **取消**.

   ![](/docs/_remote/7188871c151f9edf2199a42e.webp)

## 编辑单个穿戴物

### 编辑穿戴物属性

要编辑单个穿戴物，请按以下步骤操作：

1. 点击你想在世界中查看的项目右侧的肉丸菜单（三个水平点）。下拉菜单将出现。选择 **在编辑器中打开**.

   ![](/docs/_remote/abfed6b24f3e8e71867a0391.webp)
2. 像编辑标准穿戴物一样编辑该穿戴物。请遵循 **编辑项目** 中的 [创建穿戴物指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/) 有关如何创建自定义缩略图的说明。

### 编辑可穿戴物关联

将可穿戴物与 NFT 集合关联是 Linked Wearable 最重要的属性之一。要编辑它们与 NFT 的关联方式，你可以直接在集合视图中快速更改关联值，而无需跳转到其他页面。

![](/docs/_remote/b9bb316d91d55debb38713cb.webp)

查看“[Linked Wearables 如何表示 NFT？](#how-do-linked-wearables-represent-nfts)”部分，了解有关如何关联您的可穿戴物的更多信息。

## 批量编辑穿戴物

沿用之前在 [批量创建可穿戴物](#adding-wearables-to-the-linked-wearables-collection-in-bulk) 部分中看到的相同思路，第三方管理者可以批量修改可穿戴物。

要批量更改可穿戴物，需要为每个将被修改的可穿戴物创建一个 ZIP 文件。

这些 ZIP 文件必须按照中所述的格式创建 [批量创建可穿戴物](#adding-wearables-to-the-linked-wearables-collection-in-bulk) ，但有一个例外，在 `wearable.json` 文件中， `id` 属性 **必须** 设置为 `id` 或将被更改的可穿戴物的 URN。这是强制性的，因为这是识别待更改可穿戴物的唯一方式。如果你在创建可穿戴物时通过在 `wearable.json` 文件中提供了 ID，你就可以重新使用它们的 `wearable.json` 文件来更新它们。

考虑到中的示例 [批量创建可穿戴物](#adding-wearables-to-the-linked-wearables-collection-in-bulk) 部分，如果我们想更改某个可穿戴物的一些属性，例如我们忘记在名称后添加数字，那么我们应在压缩包中包含一个 `wearable.json` 文件，如下例所示：

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "A hat 1",
  "description": "穿戴物的描述",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat"
  },
  "mapping": {
    "type": "single",
    "id": "123"
  }
}
```

其中 `id` 字段被设置为 `id` 或将被更改的可穿戴物的 URN，而 `名称` 字段被设置为可穿戴物的新名称。

准备好 ZIP 文件后，请按以下步骤批量编辑这些物品：

1. 点击按钮组最右侧的肉丸菜单（三个水平点）。下拉菜单将出现。选择 **批量编辑**.

   ![](/docs/_remote/0c8a87dd214ea40c8a6ed252.webp)
2. 一个类似于该部分中的模态窗口 **批量上传模型** 将会出现。点击 **浏览你的电脑** 链接以打开你的文件管理器。

   ![](/docs/_remote/a39bb531aa4d0dbae170bd92.webp)
3. 选择所有将被编辑的物品的 ZIP 文件。

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. 检查所有文件是否正确，或者是否需要修复。在这种情况下，要么是穿戴物的模型未设置，要么 `wearable.json` 文件的表示形式设置不正确。

   ![](/docs/_remote/4a7baded9874d3cc155f2fbd.webp)
5. 通过点击 **添加更多** 按钮并重新上传同名失败文件，或者使用模态框右上角的垃圾桶图标忽略错误，从而修复任何错误。

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. 通过点击 **上传项目**.

   ![](/docs/_remote/42f683d2a6f504f05485aec8.webp)
7. 进行所有穿戴物的上传。请耐心等待，这可能需要一些时间！

   ![](/docs/_remote/5894f0726ee565293b5d41a2.webp)
8. 成功！你的物品现已可在你的集合中使用。请查看论坛帖子以获取策展人的任何更新。

   ![](/docs/_remote/fb076a01ef98b4db599d68cf.webp)

## 发布你的 Linked Wearables

你的 Linked Wearables 也需要像常规可穿戴物一样经过发布和策展流程。虽然发布和策展流程与常规可穿戴物的流程不同，但它保持相同步骤，所有物品都必须先发布，之后再由指派的策展人进行策展。

以下各节将向你展示如何发布你的 Linked Wearables 以便进行策展。

## 费用

创建 Linked Wearables 的费用取决于你选择构建的 Linked Wearable 集合类型：

1. **标准**：每个已发布的可穿戴物费用与常规可穿戴物相同：100 美元，可用 MANA 支付。
2. **程序化**：一笔固定费用，仅需为你将要发布的所有可穿戴物支付一次。该费用相当于发布 20 个常规可穿戴物：2000 美元，可用 MANA 支付。

有关你正在创建的集合类型的更多信息，请查看 [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) 部分。

## 发布可穿戴物以供审核

一旦你的可穿戴物准备就绪，就必须发布以供策展。你的可穿戴物会按物品组进行发布，你可以通过选择它们并点击 `发布` 按钮来选择哪些物品已准备好进行策展。发布物品后，在已发布的物品完成策展之前，发布将被阻止。

要发布你的可穿戴物，你需要：

1. 通过点击旁边的复选框选择要发布的物品。准备好选择后，点击 **发布** 按钮。

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
2. 确认你的集合名称。一旦你发布了物品，就无法更改集合名称，因此请务必仔细检查。

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
3. 再次检查你要发布的物品。点击 **确认物品** 按钮，当你准备好时。

   ![](/docs/_remote/8a8e23ad31bfa01cd5132c54.webp)
4. 在你的钱包中签署发布物品的确认。

   ![](/docs/_remote/88c6a6a00343c5fac618b93d.webp)
5. 阅读并检查条款和条件。

   ![](/docs/_remote/5eeaa4e5bf1e5c04ddcfe174.webp)
6. 检查你的发布费用。发布可穿戴物所需的费用说明在 [费用部分](#costs).

   ![](/docs/_remote/763b602c672547a4d2a6101b.webp)
7. 如果这是你第一次发布 Linked Wearables，你需要授权 Linked Wearables 智能合约代表你操作 MANA。这一步是为了从你的钱包中扣除用于支付发布费用的 MANA。

   ![](/docs/_remote/86176e8cb7ef6bc5bbe8c139.webp)
8. 支付发布费用，并通过执行交易完成发布。根据网络拥堵情况，这可能需要一些时间。

   ![](/docs/_remote/bdbd4028179d589523236310.webp)
9. 成功！你已发布你的物品。你的物品将像常规集合一样经过策展流程。你可以通过论坛帖子与策展人沟通。

   ![](/docs/_remote/1c4b8bde3d0569bee971bd77.webp)

## 提交更改以供审核

已发布并获批的可穿戴物在编辑后需要再次经过策展流程。别担心，已发布的可穿戴物不会收取任何费用。

要提交更改以便它们接受策展，你需要：

1. 通过点击旁边的复选框选择有更改的物品。点击 **提交更改** 按钮。

   ![](/docs/_remote/2cb347394e74593c7becf04c.webp)
2. 继续提交更改流程。正如模态窗口所说，这些更改之后还需要再次经过策展阶段。

   ![](/docs/_remote/3a60e8ad8fb155002a8e4cef.webp)
3. 阅读并检查条款和条件。接受后，这些物品将再次准备好接受策展。

   ![](/docs/_remote/14b970d046bc0d27c4efbb13.webp)

## 策展

与常规可穿戴物一样，你的 3D 模型需要获得策展委员会的批准。你并不例外于这条规则，因为 Decentraland 的美学和玩法仍需得到保护。

策展流程会根据用于生成可穿戴物的流程而有所不同。Linked Wearables 集合支持手工制作和程序化生成的可穿戴物。

### 手工制作的可穿戴物

对于那些单独制作且未使用任何自动化流程的 3D 模型（这是大多数常规可穿戴物的常用方法），策展人需要逐个查看集合中的所有物品，以确保它们都符合 [可穿戴物指南](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/).

### 程序化集合

对于程序化集合，并非所有物品都必须逐一策展。每个集合中需要被策展的物品数量取决于集合的规模，这由 DAO 在中定义 [该提案](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/linked-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
