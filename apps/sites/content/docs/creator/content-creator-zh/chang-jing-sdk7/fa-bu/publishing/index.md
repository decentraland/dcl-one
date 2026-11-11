> 如何发布我的项目？

# 发布

## 开始之前

请确保以下事项：

* 您的场景符合所有 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)。其中大多数会在您每次运行场景预览时进行验证。
* 如果您希望场景在移动端玩家上运行良好，请先在真机上预览并测试——参见 [面向移动设备构建](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 和 [在移动设备上预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/)。一旦发布，您的场景可通过 Decentraland 移动应用访问，方式与桌面端相同。
* 您拥有一个 [MetaMask](https://metamask.io/) 账户，并且您的 LAND 地块已分配到该账户。
* 您拥有所需数量的相邻 LAND 地块，或一个 Decentraland NAME。否则，您可以在 [市场](https://market.decentraland.org) 或在 [交易市场](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> \*\*📔 注\*\*：多地块场景只能部署到相邻地块。



> [!CAUTION]
> **❗警告**：在策划直播活动时，不要在活动开始前最后一刻才更改场景。请至少在活动开始前一小时发布最终版本。
> 
> 每次发布后，内部流程会在所有 3D 模型可渲染之前对其进行优化。这通常只需几秒钟，但对于非常大的场景或服务器繁忙时可能需要更久。在完成之前，玩家看到的仍是您场景的上一版本。


## 检查场景数据

在部署时，CLI 会从 *scene.json* 文件中读取信息，以确定将场景部署到哪里。

打开您的场景的 *scene.json* 文件，并填写以下数据：

* **标题**：当玩家进入您的场景时，标题会显示在小地图下方的界面中，也会显示在传送弹窗中。
* **描述**：对玩家在您的场景中会看到什么的描述。它会显示在传送弹窗中。
* **导航地图缩略图**：代表您场景的图片。它会显示在传送弹窗中。该图片应为 *.png* 或 *.jpg* 比例为 16:9 的图片，建议尺寸为 *1920x1080* 像素。请将任何重要内容保留在中央 *1080x1080* 正方形区域内，因为平台的某些部分会显示其方形裁剪版本。参见 [场景缩略图](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#scene-thumbnail).
* **地块**：您的场景将占用的地块坐标
* **基点**：将被视为场景 \[0,0] 坐标的地块坐标。如果您的场景包含多个地块，则应为左下角（西南）的地块。
* **生成点**：场景内的一组坐标（相对于场景的基准地块），玩家将在此生成。默认情况下，玩家会生成在 *0,0,0* 场景的位置（左下角）。可使用此项将玩家从特定位置开始，或设置一个区域以防止玩家首次出现时彼此重叠。
* **标签**：用于描述您场景的一组标签。这有助于玩家探索 Decentraland，并更容易找到他们感兴趣的内容。 [查看可用分类列表](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#categories).
* **分级**：这用于对您的场景内容进行分类。Decentraland 是一个 18+ 平台，因此应设置的值为 `A` 适用于成人。参见 [年龄分级](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#age-rating).


> [!WARNING]
> **📔 注**：参见 [场景元数据](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/) 以了解如何设置这些参数的更多详情。


## 要发布场景

### 在 Creator Hub 中使用场景编辑器

Creator Hub 中的场景编辑器提供了发布场景的便捷方式。请确保您已 [安装 Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/).

1. 打开您的场景项目。
2. 点击 **发布** 按钮，位于右上角。
3. 系统会提示您是否要发布到 **WORLD** 或到 **LAND**.
   * 选择 **发布到 WORLD** 以便将您的场景发布到您的某个 [WORLD](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds)。然后选择要发布到哪个 NAME 或 ENS 域名。
   * 选择 **发布到 LAND** 如果您拥有土地，或已被所有者授予部署权限。然后在地图上选择要部署到的地块。允许部署的地块会以粉色显示。
   * 选择 **替代服务器** 发布到 [自定义服务器](#custom-servers).

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

4. 这会在浏览器中打开一个新标签页，显示上传详情。请使用您的钱包对部署进行签名。这只是用于证明所有权的签名，免费且不涉及任何区块链交易。
   * 对于在浏览器中使用 MetaMask 账户管理的 LAND，请确认部署，然后在 MetaMask 浏览器扩展中签署消息。
   * 对于关联到可通过 Wallet Connect 使用的钱包的 LAND，请点击 **连接钱包**，然后用您的移动设备扫描二维码，并按照 Wallet Connect 中的步骤操作。

### 通过 CLI

1. 使用与您在 Decentraland 地块关联的同一公钥地址登录您的 MetaMask 账户。
2. 运行 `npm run deploy` 于场景文件夹中。


> [!NOTE]
> \*\*💡 提示\*\*：如果您的项目文件夹中有不想部署的文件，请在部署前将它们列入 \_.dclignore\_ 文件中。


3. 浏览器会打开一个标签页，显示您要部署到哪些地块。点击 **签名并部署**.
4. MetaMask 会打开，提示需要您的签名。点击 **签名** 以确认此操作。


> [!NOTE]
> **💡 提示**：如果您正在实现持续集成流程，使场景更改自动部署，那么您可以将 `export DCL_PRIVATE_KEY` 环境变量设置为拥有部署权限的账户私钥。



> [!NOTE]
> **💡 提示**: `npm run deploy` 会运行一次 `npm run build`，它对场景类型错误的检查比运行 `npm run start`更严格。如果这些错误无法避免（例如：它们发生在外部库中）且不会影响场景，您可以使用 `npm run deploy -- --skip-build` 来跳过 `npm run build` 这一步，并按原样部署场景。


## 发布到 Worlds

要将您的场景发布到 Decentraland World，您需要拥有一个 Decentraland NAME 或 ENS 域名。参见 [发布选项](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 了解如何获取它的相关信息。

### 配置 scene.json

您需要指定在什么 **名称** 下进行部署。请在您的 `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth"
	}
}
```

该 **名称** 在中指定的 `scene.json` 可以是 Decentraland NAME 或 ENS 域名。它必须由签署部署的钱包拥有，或由通过访问控制列表（ACL）授予权限的任意钱包拥有。

请注意以下事项：

* 签署部署的钱包必须拥有在 `scene.json` 文件中指定的 NAME
* 场景没有地块限制（自 2023 年 1 月起）
* 除非您按下文所述选择退出，否则所有 Worlds 都会自动列在 Places 页面上

### World 元数据

World 有自己的名称、描述和缩略图，独立于发布到其中的场景元数据。当您发布到一个空的 World，或发布到一个仅包含单个场景的 World 时，World 的元数据会在每次发布时更新为与场景元数据一致。如果该 World 包含多个场景，则其元数据是独立的：发布场景不会更改它，只能在 Creator Hub 的 **管理** 部分中编辑。参见 [World 元数据与场景元数据](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#world-metadata-vs-scene-metadata).

### Worlds 存储额度

部署到 Worlds 的场景会计入与您钱包拥有的所有 Worlds 共享的存储额度。该额度会随着您持有的 Decentraland 资产增加而增长：

* 您拥有的每个 Decentraland NAME 可获得 100 MB。
* 您拥有的每个 LAND 地块可额外获得 100 MB。
* 您钱包中每持有 2,000 MANA 可额外获得 100 MB。

您可以自由地在各个 World 之间分配这份额度。例如，若额度为 500 MB，您可以部署一个 300 MB 的场景和一个 200 MB 的场景，或者五个 100 MB 的场景。发布到 ENS 域名的 Worlds 不消耗此额度，而是每个 World 固定限制为 36 MB。

您可以在两个地方查看已使用和剩余的额度：

* 该 **管理** Creator Hub 的 **查看详情** 以查看您的 MANA、LAND 和 NAME 持有量如何汇总的明细。
* 该 **Worlds** 的选项卡 [构建器](https://decentraland.org/builder/worlds).

如果您超出额度（例如在出售或转移资产后），您有 48 小时来释放空间或增加额度，否则您的 Worlds 将变得无法访问。参见 [Worlds 大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#size-limits) 了解更多详情。

### 多场景 worlds

默认情况下，通过 CLI 部署到 world 时，您的场景将被视为单场景 world。如果您的 world 已经包含一个场景，部署新场景会覆盖旧场景，即使坐标不同也是如此。

要作为多场景 world 部署，请在部署命令中添加参数 `--multi-scene` ：

`npm run deploy -- --multi-scene --target-content https://worlds-content-server.decentraland.org`

使用此参数时，如果您正在发布的场景与该 world 中现有场景的坐标不同，这些场景将会同时存在。


> [!WARNING]
> \*\*📔 注\*\*：如果您对一个并非自己拥有的 world 拥有操作权限，而且只被授予该 world 中特定地块的权限，那么您只能发布到这些地块，且无法影响该 world 其他地块的内容。


### 退出 Places 列表

如果您希望您的 Worlds 不被 Places 索引，可以在您的 `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

### 在 Creator Hub 中使用场景编辑器

1. 打开您的场景项目。
2. 点击 **发布** 按钮，位于右上角。
3. 选择 **发布到 WORLD**.
4. 选择要发布到哪个 NAME 或 ENS 域名。

### 通过 CLI

使用以下命令：

```bash
npm run deploy -- --target-content https://worlds-content-server.decentraland.org
```

运行该命令后，系统会提示您使用钱包对部署进行签名，并执行一系列验证以决定是否允许或拒绝该场景。


> [!WARNING]
> **📔 注**：当您的 `scene.json` 有一个 `worldConfiguration` 部分时， `npm run deploy` 需要 `--target-content` 标志（或 `--target`）。如果没有这两个参数中的任意一个，命令将以错误 `DEPLOY_MISSING_TARGET_CONTENT`.


### 访问 World

场景上传到 Worlds 服务器后，您可以使用 Decentraland Explorer 通过以下 URL 访问： `decentraland://?realm=NAME.dcl.eth`，其中 `NAME` 应替换为您部署到的 Decentraland NAME 或 ENS 域名。

在 Decentraland 已打开的情况下，您也可以通过输入以下内容跳转到某个 world： `/goto NAME.dcl.eth` 命令到聊天框中。

参见 [使其可被发现](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/make-discoverable/) 了解如何让您的 World 可被发现的更多信息。

### 将 World 迁移到 Genesis City

如果您是 LAND 所有者，并希望将 World 场景部署到 Genesis City，您可以通过将场景重新部署到去中心化的 Catalyst 网络来实现，该网络是 Genesis City 的内容服务器。

需要记住的事项：

* 删除 `worldConfiguration` 中的部分 `scene.json`
* Worlds（基于持有量动态变化）的大小限制与 LAND 地块（每个地块 15MB）的限制不同，因此请确保您的场景大小适合部署到 Genesis City！

## 发布大型场景

大型场景会分批上传，因此您可以将大于约 200 MB 的场景发布到 Genesis City 或 World。这需要 `@dcl/sdk` 您的项目中安装 7.31.0 或更高版本。Creator Hub 会使用您项目中安装的 SDK 进行发布，因此在通过 Creator Hub 发布时也同样适用。

上传方式如下：

* **小于 50 MiB** （约 52 MB）的文件总量：场景会像以前一样通过单个请求上传。
* **50 MiB 或更多**：文件将按每批 32 MiB、每次三批的方式上传。单个文件不会被拆分到多个批次中。如果某个文件大于 32 MiB，批次会自动扩展以容纳它。
* 每个批次后，CLI 会打印一行进度，例如 `已上传 96.0 MB / 268.8 MB（第 3/9 批）`.

分批不会改变大小限制。每个文件仍必须小于 50 MB，且整个场景必须符合 [总文件大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 对应地块的限制，或者您的 [Worlds 存储额度](#worlds-storage-budget).


> [!WARNING]
> **📔 注**：如果您部署到的服务器不支持分批上传，例如在使用 [自定义服务器](#custom-servers)时，CLI 会检查服务器缺少哪些文件。如果这些文件总计不超过 200 MB，就会显示警告 `目标服务器不支持分批上传，正在以单个请求上传。` 并将它们一次性上传。如果总量超过该值，部署将失败并返回错误 `DEPLOY_PARTIAL_NOT_SUPPORTED`。单个请求不能超过 200 MB，因为内容服务器前端的基础设施会对其超时。


### 取消部署

当您运行时打开的浏览器标签页 `npm run deploy` 会显示上传进度，并带有一个 **取消** 按钮。取消会停止上传。如果在取消请求到达时服务器已经发布了场景，则部署会以已发布状态结束。


> [!CAUTION]
> **❗警告**：当您发布到一个没有 `--multi-scene`，且该 World 在其他地块上有场景时，CLI 会在上传您的场景之前移除那些场景（参见 [多场景 worlds](#multi-scene-worlds)）。如果您在它们被移除后取消，World 可能会被留空，CLI 会以错误 `DEPLOY_CANCELLED_AFTER_UNDEPLOY`。请重新部署以发布场景。


如果 CLI 无法确认服务器是否已发布场景，则会以错误 `DEPLOY_CANCEL_OUTCOME_UNCERTAIN`。请在再次部署前检查场景。

无需浏览器标签页即可运行部署的工具，可以通过带有令牌的部署 API 取消部署。CLI 每次运行时都会创建一个随机令牌。要设置您自己的令牌，请传入 `--deploy-token` ，其值至少为 16 个字符：

```bash
npm run deploy -- --deploy-token my-own-deploy-token-0001
```

使用 `--programmatic`时，命令结果中会将该令牌包含为 `deployToken`。运行 `npx sdk-commands deploy --help` 以查看部署 API 路由。

### 部署错误代码

| 错误代码                              | 含义                                                                |
| --------------------------------- | ----------------------------------------------------------------- |
| `DEPLOY_PARTIAL_NOT_SUPPORTED`    | 服务器不支持分批上传，并且其缺少的文件超过 200 MB。                                     |
| `DEPLOY_CONTENT_CHECK_FAILED`     | CLI 无法检查服务器已有哪些文件。只有当服务器不支持分批上传且场景超过 200 MB 时，才会运行此检查。            |
| `DEPLOY_CANCELLED`                | 部署已取消，场景未发布。                                                      |
| `DEPLOY_CANCELLED_AFTER_UNDEPLOY` | 部署在移除了该 World 先前的场景后被取消。                                          |
| `DEPLOY_CANCEL_OUTCOME_UNCERTAIN` | 部署已取消，但 CLI 无法确认服务器是否已发布它。                                        |
| `DEPLOY_MISSING_TARGET_CONTENT`   | 场景有一个 `worldConfiguration` 但命令没有 `--target-content` 或 `--target`. |

## 从硬件钱包发布

与其将您的 LAND 代币存放在 MetaMask 账户中，您也许会觉得将它们存放在硬件钱包设备中更安全，例如 [Ledger](https://www.ledger.com/) 或 [Trezor](https://trezor.io/)，该设备通过物理方式连接到您的电脑。

如果您正在使用这些设备之一，您可以将硬件钱包连接到 MetaMask，以启用消息签名，同时让代币更安全。参见 [MetaMask 的这篇文章](https://metamask.zendesk.com/hc/en-us/articles/360020394612-How-to-connect-a-Trezor-or-Ledger-Hardware-Wallet) 获取连接账户的说明。

一旦您的硬件钱包可以通过 MetaMask 使用，您就可以按照与代币存放在 MetaMask 账户中相同的步骤进行部署。

## 场景覆盖

当新场景部署时，它会覆盖其所占地块上原先存在的旧内容。

如果一个占用多个地块的场景只是被另一个场景部分覆盖，那么它的所有地块要么被覆盖，要么被清除。

假设您将场景部署在 *A* 两个地块上 *\[100, 100]* 和 *\[100, 101]*。然后您将地块 *\[100, 101]* 卖给了一个拥有相邻土地的用户，而该用户又部署了一个大型场景（*B*）到多个地块上，包括 *\[100, 101]*.

您的场景 *A* 无法只在一个地块中部分渲染，因此 *\[100, 100]* 不会显示任何内容。您必须构建场景的新版本 *A* ，使其只占用一个地块，并且仅部署到地块 *\[100, 100]*.

## 什么是内容服务器

内容服务器是由社区拥有的一组服务器网络，其文件系统采用内容寻址方式，这意味着每个文件都是通过其内容而不是任意文件名来标识的。

我们使用内容服务器以类似 BitTorrent 的方式托管和分发所有场景内容，从而保持 Decentraland 网络的分布式特性。

1. 内容服务器存储并分发渲染您场景所需的全部资产。
2. 该 `npm run deploy` 命令将这些资产链接到您在中指定的 LAND 地块 **scene.json** 文件中。每当您重新部署场景时，内容服务器都会更新记录，使地块指向最新内容。部署不涉及任何区块链交易，您只需使用钱包对新的场景实体签名以证明所有权。

服务器每个副本上的信息都是可验证的，因为每个场景都由 LAND 所有者的哈希签名。这意味着托管服务器副本的人无法篡改内容来显示不合法的内容。社区也可以通过 DAO 投票批准或移除这些服务器中的任何一个。

## 自定义服务器

您可以将内容部署到不属于官方 DAO 维护的 catalyst 服务器网络的自定义服务器。为此，您不需要拥有任何 LAND 或 NAME 代币，因为您可以将服务器配置为使用您偏好的任意验证逻辑，以控制谁可以在哪里部署。自定义服务器可以选择包含来自官方服务器的内容（您可以覆盖这些内容），也可以从空白开始并发布全新的内容。

参见 [catalyst-owner 仓库](https://github.com/decentraland/catalyst-owner) 以了解有关您自己的服务器可以做什么以及如何设置它的更多信息。


> [!WARNING]
> **📔 注**：玩家需要手动输入 URL 才能访问您的自定义服务器。来自诸如 [奖励服务器](/docs/creator/content-creator-zh/jiang-li/getting-started/) 的某些验证在这些情况下可能会失败，因为这些服务通常要求请求来自官方服务器。


玩家不会被自动引导到该服务器，访问它的唯一方式是显式输入 URL 以连接到它。

### 通过 CLI

要部署到自定义服务器，请运行：

`npm run deploy -- --target <CUSTOM SERVER DOMAIN>`

要在自定义服务器上进入您已部署的场景，所用 URL 将取决于其托管的域名。

## 验证部署是否成功

在您部署场景后，这些更改可能需要几分钟才能传播到网络中的各个内容服务器。如果您在部署后立即进入 Decentraland，可能会短暂地仍看到您内容的上一版本。

如果使用 CLI 部署，签名 dapp 会开始显示确认信息，表示您内容的新版本已传播到网络中的所有服务器。

您将看到构成 Decentraland 内容网络的每台服务器的列表。对于每台服务器，它会注明该地块上最近一次上传更改的时间戳。每台服务器对应不同的 realm。您可以在 [catalyst monitor 屏幕](https://decentraland.github.io/catalyst-monitor/).

传播只是整体的一部分：上传完成后，场景的 3D 模型会被转换为针对各个平台（Windows 和 Mac）优化的资源包。在转换完成之前，玩家有意会看到您场景最后一个完全可用的版本。这通常只需几秒钟，但对于非常大的场景或服务器繁忙时可能需要更久。您可以直接查询转换进度，参见 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status).

您还可以随时在命令行控制台中运行以下命令来获取传播信息：

`npx @dcl/opscli pointer-consistency --pointer 0,0`


> [!WARNING]
> **📔 注**：请使用你场景的坐标，而不是 `0,0`。如果你的场景有多个地块，任意一个地块都会产生相同的输出。如果坐标以负数开头，请在前面添加一个 `\` 放在坐标开头，以防止 `-` 该字符被命令行误解释。


## 自动部署

如果你经常更改场景内容，并希望实现简化的发布流水线，可以通过 GitHub Action 自动部署你的场景。

为此，你必须先将你的场景存储在 GitHub 项目中。然后你可以设置一个 [GitHub Action](https://docs.github.com/en/actions) ，使用以下脚本，它会在每次合并到 `main` 分支时运行。该脚本会安装所有依赖项，构建项目，然后将其部署到 Decentraland。

```yaml
name: 部署到 DCL PROD

on:
  push:
    branches:
      - main

env:
  DCL_PRIVATE_KEY: ${{ secrets.DCL_PRIVATE_KEY }}

jobs:
  deploy:
    name: 部署
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: 安装 npm 包
        run: |
          npm install
      - name: 构建场景
        run: |
          npm run build
      - name: 部署场景
        run: |
          npm run deploy
```

> 重要：要运行此流程，你必须在 GitHub 中将某个钱包的私钥设置为环境变量，该私钥用于签署部署。和往常一样，请务必小心保管私钥安全。请不要使用实际拥有土地代币的账户的私钥，因为这会带来严重的安全风险。相反，应将操作员权限委托给一个不持有任何有价值代币的临时钱包。如果这个私钥泄露了，你可以轻松撤销该账户的这些操作员权限，并设置一个新钱包。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
