> 如何将你的场景发布到 LAND 或 NAME。

# 发布场景

## 开始之前

请确保以下几点：

* 你的场景符合所有 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)。其中大多数限制会在你每次运行场景预览时进行验证。
* 你拥有一个 [MetaMask](https://metamask.io/) 账户，并且你的 LAND 地块或 NAME 已分配到该账户。
* 你拥有所需数量的相邻 LAND 地块或一个 Decentraland NAME。否则，你可以在 [Marketplace](https://decentraland.org/marketplace/) 购买 LAND，或在 [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> **📔 注**：多地块场景只能部署到相邻地块。


检查你的 [场景详情](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/#scene-details)，确保你提供了吸引人的名称、描述、缩略图、类别等。


> [!CAUTION]
> **❗警告**：在规划线下活动时，不要在活动开始前的最后一刻修改场景。请至少在活动开始前一小时发布最终版本。
> 
> 在你更新之前，已经在当前会话中加载过该场景的玩家仍会继续看到该场景的缓存版本。要看到更新后的版本，他们必须关闭并重新进入 Decentraland。


## 发布你的场景

要发布你的场景：

1. 在 Scene Editor 中打开你的场景并点击 **发布**。这会打开一个显示发布详情的窗口。
2. 选择你要发布到 LAND 还是 WORLD。查看 [项目类型](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/) 以更好地理解不同选项。

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

3. 如果发布到 LAND，请在地图上选择位置。你会看到符合条件的地块以红色标记。如果发布到 WORLD，你会在下拉菜单中看到符合条件的 NAME。


> [!NOTE]
> **💡 提示**：如果你没有看到你的地块或 NAME，请确保你使用正确的用户账户连接到了 Creator Hub。否则请退出项目，点击右上角的用户设置图标，然后选择 **退出登录** 并重新登录。


4. 下一屏会显示你当前正在上传的所有文件及其大小，请确认操作。
5. 随后发布过程将开始。它分为两个阶段，两个阶段都完成后会出现一个 **进入** 按钮。


> [!NOTE]
> **💡 提示**：成功页面还会显示一个指向你场景的网页链接，并带有一个复制按钮。任何人都可以在浏览器中打开该链接，无论他们是否安装了桌面客户端，因此它是最适合粘贴到 Discord 消息或活动页面中的链接。它看起来像 `https://decentraland.org/jump?realm=<your-world-name>` 用于 World，或 `https://decentraland.org/jump?position=<x>,<y>` 用于 Genesis City 中的场景。
> 
> 该 **进入** 按钮是给你自己用的，而不是给观众用的：它会在你自己的桌面客户端中打开场景，这样你就可以检查你的作品。


![](/docs/_remote/a76ebe12a85e91368d25d90d.webp)


> [!NOTE]
> **📔 注**：部署的两个阶段包括：
> 
> * **1. 上传**：将文件上传到服务器。50 MiB 或更大的场景会分批上传，参见 [发布大型场景](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/#publish-large-scenes).
> * **2. 转换**：场景的 3D 模型会被压缩为 Asset Bundle，以便更快渲染。这通常只需要几秒钟。对于非常大的场景可能会更久。
> 
> Decentraland 的服务器还会生成低细节层级（LOD）版本的模型，用于从远处绘制你的场景。这会在你的场景已经上线后于后台运行，因此发布过程不会等待它完成。


## 发布需要多长时间？

* **直到你可以测试它**：从几秒到几分钟不等。它取决于你的场景总大小和上传带宽。两个阶段都完成后， **进入** 按钮会在场景可玩时立即出现。
* **在线下活动之前**：请至少在活动开始前一小时发布最终版本，以便留出应对意外情况的余地。

以下因素会使发布变慢：

* **场景总大小**：会同时影响上传和转换。
* **不同 3D 模型的数量和复杂度**：模型会逐个转换。
* **转换服务器有多繁忙**：你的场景可能会排在同一时间发布的其他场景后面。

## 管理 Worlds

Creator Hub 通过其主面板中的 **管理** 选项卡来支持 World 管理。 **管理** 该选项卡可用于跟踪和编辑 World。在这里，你可以编辑 World 设置、权限和场景。

### World 存储预算

发布到 World 的场景会计入一个存储预算，该预算由你钱包拥有的所有 World 共享。预算会随着你的持有量增长：你拥有的每个 Decentraland NAME 或 LAND 地块可获得 100 MB，而你钱包中每持有 2,000 MANA 还可额外获得 100 MB。发布到 ENS 域的 World 则有固定的 36 MB 上限。

该 **管理** 选项卡显示你正在使用多少存储预算。点击 **查看详情** 以查看你的 MANA、LAND 和 NAME 持有量如何累计。

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

你也可以在 **Worlds** 选项卡中查看剩余预算的情况，位于 [Builder](https://decentraland.org/builder/worlds).

查看 [World 大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#size-limits) 以了解预算是如何计算的，以及超出预算后会发生什么。

### World 设置

World 所有者可以进入所需的 World **设置** 面板中编辑其设置，或者在发布过程中通过点击 **管理** 来访问它，前提是 **设置** 如果 **多场景 World（高级）** 已启用。

<img src="/docs/_remote/c29f510065b494f433726156.webp" alt="" width="600">

* **详情**：World 的基本信息：
  * World 标题
  * 描述
  * 内容分级
  * 类别

在 **详情** 中添加的信息在发布后会显示在 Decentraland Places 和 World 内的信息中。请记住，如果该 World 只包含一个场景，这些详情会在每次发布时被场景的元数据覆盖。请参见 [World 元数据与场景元数据](#world-metadata-vs-scene-metadata) 如下。

* **布局**：仅可在多场景 World 中访问。包含有关该 World 所有已发布场景的信息。
  * 通过点击三点菜单并选择 **从 World 中移除**.
  * **World 地图** 显示 World 的布局，并标识出包含内容的地块以及剩余空闲地块。

<img src="/docs/_remote/f965a7cd250c7251c2c60acb.webp" alt="" width="600">

* **其他**：其他有用的 World 配置：
  * World 生成坐标：这会设置用户在 World 中出生的地块 (X,Y)。位于该地块中的场景决定用户出生的精确位置（例如，地块 1,1 是 World 生成点，而 1,1 中的场景具有一个 1,0,1 的生成点 **在该场景内**).
  * 天空盒设置


> [!NOTE]
> **📔 注**：World 设置仅可由 World 所有者（铸造该 NAME 的地址）访问。有关如何获取 NAME 的更多详情，请查看 [Marketplace 的 NAME 部分](https://decentraland.org/marketplace/names/claim).


### World 元数据与场景元数据

一个 World 和发布到其中的每个场景都有两套彼此独立的元数据。两者都包含名称、描述和缩略图：

* **场景元数据**：属于场景项目，存储在其 `scene.json` 文件中。你可以通过 [场景设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/) 在 Scene Editor 中编辑它，并且每次发布时它都会与场景一起上传。
* **World 元数据**：属于 World 本身。你可以在 World 的 **设置**中编辑它，位于 **管理** Creator Hub 的选项卡中。这些信息会显示在 Decentraland Places 和 World 内的信息中。

这两者之间的关系取决于该 World 包含多少个场景：

* **空 World**：当你将一个场景发布到空 World 时，World 的元数据会根据该场景的元数据填充。
* **单场景 World**：每次你发布该场景时，World 的元数据都会更新为与该场景的元数据一致。
* **多场景 World**：World 的元数据与其每个场景的元数据完全独立。发布场景不会改变 World 的元数据。更改它的唯一方式是通过 World 的 **设置** 在 **管理** 选项卡中。

实际上，如果你的 World 只包含一个场景，你不需要把 World 的元数据视为独立的东西：只需编辑场景设置并发布，World 就会始终反映这些设置。


> [!WARNING]
> **📔 注**：在单场景 World 中，你对 World 的 **设置** 在 **管理** 选项卡所做的任何更改都会在你下次发布场景时被场景元数据覆盖。若想保留这些更改，请改为编辑场景设置。


在多场景 World 中，World 的 **设置** 在 **管理** 选项卡是编辑 World 元数据的唯一位置。

### 多场景 Worlds

一个 World 可以有多个场景，由 World 所有者或其他创作者发布。这支持一个协作环境，其中每个地块都可以由不同的协作者管理。

#### 将 World 设为多场景

World 所有者可以通过切换 **多场景 World（高级）** ，在向单场景 World 发布时，将 World 设为多场景。

<img src="/docs/_remote/9f653359d5337efaf125df54.webp" alt="" width="600">

一旦多场景 World 发布，World 所有者就可以发布额外的场景，或添加协作者在该 World 内发布。


> [!NOTE]
> **📔 注**：多场景 World 的大小会自动调整以容纳所有已发布场景，并在每次发布时动态增减。多 World 中不同场景之间剩余的空间会被环境填充。


#### 向多场景 World 添加协作者

在 **管理** 面板中，World 所有者可以通过点击三点菜单访问 World 的 **权限** 。World 所有者可以在 **协作者** 选项卡中。

<img src="/docs/_remote/2586c8bab86636ee8a2193fe.webp" alt="" width="600">

下管理协作者。协作者可以获得所有地块的部署权限，或者仅限于特定自定义坐标。自定义坐标可以通过交互式 World 地图进行选择并确认，类似于 World 设置中的地图。

<img src="/docs/_remote/ef6b0e88960c90ce2923d1fe.webp" alt="" width="600">

#### 作为协作者向多场景 World 部署

World 协作者不能编辑其设置或权限。在 **管理** 选项卡中，创作者可以看到自己作为协作者所在的 World，但不能访问 **设置** 或 **权限**.

<img src="/docs/_remote/f15119b09e3098fe4efe4dbe.webp" alt="" width="600">

在发布过程中，创作者可以选择只发布到其作为协作者拥有权限的地块（由 World 所有者设置）。

在 **协作者** 部分中，如果 World 所有者为创作者设置了 **自定义坐标** ，则只有分配的地块可用于发布。如果访问权限被设为 **所有地块**，创作者就可以选择 World 中的任意地块来发布其场景。


> [!WARNING]
> **📔 注**：拥有 **所有地块** 发布权限的协作者可以覆盖该 World 中的任何场景，即使它是由所有者或其他协作者发布的。


<img src="/docs/_remote/4a5bbf90cf2c56710137c17e.webp" alt="" width="600">

### 私有 Worlds

一个 WORLD 可以有不同的 **访问** 设置。它可以对任何人开放，或以不同方式受限。

#### 设置 WORLD 的访问权限

在 **管理** 面板中，World 所有者可以通过点击三点菜单访问 World 的 **权限** 通过点击三点菜单。World 所有者可以在 **访问** 选项卡中。

**访问类型**

下管理访问限制。World 所有者可以在三种 **World 访问**:

**公开**

任何人都可以访问该 World。这是 World 的默认设置。

**密码保护**

只有拥有密码的用户才能进入该 World。

密码必须至少 8 个字符长，并且至少包含 2 个数字。创建后，密码将无法再次访问，因此请务必保留一份副本。

**仅限邀请**

只有在 **已批准地址** 中添加的地址和社区才能访问该 World。

要向 **已批准地址**中添加新的地址或社区，请按照以下步骤操作：

1. 点击 **+ 新邀请** 按钮。
2. 你可以通过三种不同方式添加地址：

* **钱包地址**：逐个添加单独的钱包。
* **社区**：搜索并添加任何公开社区。这会把 **所有社区地址** 添加到 **已批准地址**.

  <img src="/docs/_remote/cd2c518c4a9642b70cf0c060.webp" alt="" width="600">
* **导入 CSV**：使用现有的 CSV，其中包含要添加到 **已批准地址**的地址或社区 ID 列表。格式为每行一个钱包，例如：

```
0x3bA7fD92eC4a1F6B8d2E9c5A7b1D3f6C8e4A2d9F
0xA1c9E4b7D2f6C8a3B5e9F1d4A7c2E6b8D3f9C5a1
```

导入后，它会逐个跟踪每个地址，如图所示。

<img src="/docs/_remote/554211db6a545dd6cf7ee848.webp" alt="" width="600">

3. 确认后，这些地址就在 **已批准地址**.
4. 使用新的 **+ 新邀请**，地址会被添加到现有列表中，帮助 World 所有者在需要时管理和扩展该列表。

<img src="/docs/_remote/c316240a3038949beab28744.webp" alt="" width="600">

5. 单个地址或一组地址（在社区的情况下）可以通过选择 **删除** 在 **已批准地址** 部分中的三点菜单上移除。


> [!WARNING]
> **📔 注**：如果你更改 **访问** 类型，从 **仅限邀请**开始，你的 **已批准地址** 列表将被移除。请确保保留一份副本，以备将来需要。


#### 进入私有 Worlds

如果用户跳入一个没有 **公开** 访问权限的 World，会有不同的情况：

* 他们的地址在 **已批准地址**：将能够正常加入。否则，他们将收到信息，说明该 World 是 **仅限邀请**.

<img src="/docs/_remote/ac4dca8cd154a2b6ce27d48d.webp" alt="" width="300">

* 该 World 处于 **密码保护**：用户将可以输入密码。最大尝试次数为十（10）次。

<img src="/docs/_remote/65d948411619e496f8eed2f7.webp" alt="" width="300">

## 从硬件钱包发布

与其将你的 LAND 代币存放在 MetaMask 账户中，你也许会觉得把它们存放在硬件钱包设备中更安全，例如 [Ledger](https://www.ledger.com/) 或 [Trezor](https://trezor.io/)，这些设备是物理连接到你的电脑上的。

如果你使用这些设备之一，你可以将硬件钱包链接到 MetaMask，以便启用消息签名，同时让代币更安全。请参阅 [这篇来自 MetaMask 的文章](https://support.metamask.io/more-web3/wallets/how-to-connect-a-trezor-or-ledger-hardware-wallet/) 以了解如何连接你的账户。

一旦你的硬件钱包可以通过 MetaMask 使用，你就可以按照与你的代币存放在 MetaMask 账户中相同的步骤进行部署。

## 场景覆盖

当一个新场景被部署时，它会覆盖它所占用地块上原有的旧内容。

发布通常会立即开始。如果你即将替换 World 中已存在的内容，Creator Hub 会先要求你确认，并提供一个 **不再显示** 选项。

如果一个占用多个地块的场景只被另一个场景部分覆盖，那么它的所有地块要么被覆盖，要么被擦除。

假设你部署了你的场景 *A* 占用两个地块 *\[100, 100]* 和 *\[100, 101]*。然后你把地块 *\[100, 101]* 卖给一位拥有相邻土地并在其上部署大型场景（*B*）到多个地块的用户，包括 *\[100, 101]*.

你的场景 *A* 不能只在一个地块中被部分渲染，因此 *\[100, 100]* 将不会显示任何内容。你必须构建场景 *A* 的新版本，让它只占用一个地块，并将其只部署到地块 *\[100, 100]*.

## 发布到授予的土地

如果你要发布到通过资助授予给你的、由 Decentraland Foundation 拥有的 LAND，请正常点击 **发布** 按钮，然后选择 **发布到其他服务器** 在底部。然后从下拉菜单中选择 **自定义服务器** 并输入以下服务器地址： `https://linker-server.decentraland.org`.


> [!WARNING]
> **📔 注**：你必须先在布局设置的高级选项卡中手动设置场景坐标。有关更多信息，请参见 [场景设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/#layout) 。


## 自定义服务器

你可以将内容部署到不属于由 DAO 官方维护的 catalyst 服务器网络的自定义服务器。为此，你不需要拥有任何 LAND 或 NAME 代币，因为你可以配置服务器使用任何你偏好的验证逻辑，以控制谁可以在何处部署。自定义服务器可以选择使用来自官方服务器的内容，并允许你覆盖它，或者从空白状态开始，发布全新的内容。

要发布到自定义服务器，请点击 **发布** 按钮，然后选择 **发布到其他服务器** 在底部。然后从下拉菜单中选择 **自定义服务器** 从下拉菜单中选择，并输入服务器地址。

查看 [如何运行你自己的 Catalyst 节点](/docs/contributor/tutorials/how-to-run-a-catalyst/) 以了解更多关于你自己的服务器可以做什么以及如何设置它的信息。


> [!WARNING]
> **📔 注**：玩家需要手动输入一个 URL 才能访问你的自定义服务器。来自 [奖励服务器](/docs/creator/content-creator-zh/jiang-li/getting-started/) 等服务的某些验证在这些情况下可能会失败，因为这些服务通常要求请求来自官方服务器。


玩家不会被引导到这个服务器；访问它的唯一方式是显式输入 URL 连接到它。

## 验证部署是否成功

一旦你部署了场景，这些更改可能需要几分钟才能上传并处理。如果你在部署后立刻进入 Decentraland，你可能仍会短暂看到内容的旧版本。

Creator Hub 会在发布过程中显示其进度，随着它经过 **上传** 和 **转换** 阶段，并在场景可玩时立即显示一个 **进入** 按钮。

### 检查转换状态

在新版本的 3D 模型正在转换时，玩家会被刻意提供你场景最后一个完全正常工作的版本。转换最多应该只需几分钟，如果花费更久，可能说明出了问题。

你可以直接在浏览器中查询转换状态：

* `https://asset-bundle-registry.decentraland.org/entities/status/<pointer>`：将 `<pointer>` 替换为你场景的坐标之一（例如 `20,-34`），或替换为该部署的实体 ID。响应会显示每个平台在 `assetBundles`下的转换状态，以及在 `lods`下的 LOD 状态。当 `complete` 为 `true`时，场景就已经针对所有平台完全转换完成。对于 World 中的场景，请添加 World 名称作为查询参数，例如 `.../entities/status/0,0?world_name=myname.dcl.eth`.
* `https://asset-bundle-registry.decentraland.org/queues/status`：列出当前按平台等待转换的所有场景 ID。如果你的实体 ID 出现在这里，说明你的场景排在其他转换任务后面，你只需要再等待一会儿。


> [!WARNING]
> **📔 注**：如果你已经打开了 Decentraland 浏览器会话并加载过你的场景，你需要关闭并重新打开应用。否则你会一直看到缓存版本。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
