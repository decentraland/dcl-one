> 如何获取 Decentraland World 或 LAND 权限来发布你的场景

# 发布选项

要邀请其他人访问你构建的场景，你需要先在 Decentraland 中发布它。为此，你需要拥有向 Decentraland World 部署的权限，或拥有 Decentraland LAND 地块。

本文档概述了获取权限的不同方式。请参见 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 以了解在获得这些权限后如何进行发布的说明。

### Decentraland Worlds

Decentraland Worlds 是你在 Decentraland 生态系统中的专属个人 3D 空间，独立于 Genesis City 的公开 LAND 地块地图。

World 可以保持私密，也可以通过一个链接与任何人共享。一个 World 最多可同时容纳 100 位用户，你可以用你的 World 来：

* 举办活动
* 构建新的体验
* 分享作品集场景
* 在将场景部署到 Genesis City LAND 之前进行测试

### 如何获取一个 World

获取 Decentraland World 有两种方式：

* **获取一个 Decentraland NAME（推荐）。**\
  你可以通过获取一个 [Decentraland NAME](https://builder.decentraland.org/names)来获得你自己的 Decentraland World，它是 DCL 生态系统中的 NAME NFT。一个 NAME 费用为 100 MANA，并会立即为你提供一个同名 World。你可以 [购买一个新的 NAME](https://decentraland.org/shop/items?category=names) ，或者在 [Marketplace](https://decentraland.org/marketplace/names).
* **购买一个 ENS 域名。**\
  你也可以通过购买一个 [ENS 域名](https://ens.domains/)来获取一个 World。ENS 域名是一种去中心化名称，可在 Web2 和 Web3 中使用。通过 ENS 域名获得的 Worlds，每个 World 的场景文件大小上限固定为 36 MB，不受你在 Decentraland 中其他资产持有情况的影响。

一旦你拥有了 NAME 或 ENS 域名，就可以使用标准的 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 流程将场景发布到该 World。\
你也可以让其他人向你的 Decentraland World 发布内容。请查看 [管理 Worlds 文档](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#managing-worlds) ，了解更多关于多场景 Worlds 以及私有 Worlds 等其他主题的信息。

### World 存储容量

在一个 World 内，你可以使用任意数量的地块。对于 Decentraland NAME 持有者，你的场景大小限制（以兆字节为单位）取决于你拥有多少个 NAME、MANA 和 LAND 地块。请参见 [Worlds 大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#size-limits) 以了解存储容量的计算方式详情。

你可以在 Creator Hub 的 **管理** 部分，或在 **Worlds** 选项卡中的 [Builder](https://decentraland.org/builder/worlds).

### 了解更多关于 Worlds 的信息

* [作为项目类型的 Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#publish-to-worlds) - 了解大小限制以及 Worlds 与 LAND 场景的区别
* [发布到 Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/#publishing-to-worlds) - 分步发布说明
* [World 配置](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#world-configuration) - 配置天空盒、通信设置等
* [管理 Worlds](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/manage-scenes/#managing-worlds) - 可视化存储空间并管理部署
* [World 常见问题](https://github.com/decentraland/docs/tree/main/creator/scene-editor/faq/README.md) - 关于 Worlds 的常见问题

### 发布到 Decentraland LAND

Genesis City 是 Decentraland 的共享空间：由 LAND 地块组成的大型公共地图。每个 LAND 地块的大小为 **16 米 × 16 米**。一个场景可以跨越一个或多个地块构建。

如果你更愿意发布到 LAND，有三种选择：

* **租用 LAND**：购买在特定地块上部署并保留内容的权限，期限固定。请参见 [租赁](/docs/player/marketplace/rentals/).
* **购买土地**：购买土地地块，并在你希望的时间内发布你的内容。请参见 [marketplace](/docs/player/marketplace/marketplace/).
* **从所有者处获取权限**：一些土地所有者和区域管理着大片土地，如果内容符合他们的主题和他们想要创建的环境类型，他们愿意免费托管他人的内容。你可以在 [Decentraland Discord 服务器](https://dcl.gg/discord).

### LAND 权限的工作方式

LAND 代币可以管理权限，从而允许以下不同角色向特定地块或地产进行部署。

* **LAND 所有者**：在其 Ethereum 钱包中持有该代币的人。此用户始终可以部署、分配其他角色或转移该代币（除非该土地当前已被他人租用）。
* **LAND 操作员**：只要持有此角色，操作员就可以进行部署。操作员的地址由该代币引用，无需拥有任何资产。所有者可以向任意数量的地址分配或撤销操作员权限。请参见 [授予权限](/docs/player/marketplace/land-manager/#give-permissions).

在租用土地时，支付租金的租户可以选择一个地址，该地址将在租期内拥有一个 **LAND 操作员** 角色（可以是他们自己的地址，也可以是其他人的地址）。在租约有效期间，原始角色持有者都无法干预该 LAND 上的内容，也无法转移该代币。

### 如何发布你的场景

当你获得 World 或 LAND 权限后，请按照标准的 [发布指南](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 来部署你的场景。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
