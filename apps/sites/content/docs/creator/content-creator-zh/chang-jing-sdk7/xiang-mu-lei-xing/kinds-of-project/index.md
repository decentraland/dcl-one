> 你可以在 Decentraland 中创建的各种项目类型。

# 概览

你可以为 Decentraland 制作不同类型的内容。

![](/docs/_remote/0a4a8ae5cf4dbf5eb00b75db.webp)

## 场景

Decentraland 场景可以托管在 Genesis City 的地块上，或托管在 WORLDs 中。

### 发布到 LAND

发布到地块的场景可以在 Decentraland 开放世界中的特定坐标找到。

这些场景与 LAND 或 Estate 代币相关联。每个地块占地 16x16 米。单个场景可以占用多个相邻地块，这些地块可以排列成任意形状，只要边界相接即可。

LAND 代币可以在 [市场](https://decentraland.org/marketplace/lands)购买。它们的供应有限，覆盖了 Genesis City 的地图。

发布到 LAND 的场景更容易被发现，因为玩家在访问附近内容或探索时可能会偶然遇到它们。

发布到 LAND 的场景在场景中的每个地块最多可使用 15 MB 空间。地块越多，可用空间越大。这样做是为了防止玩家的 CPU 负载过高，因为玩家可能会同时体验到许多附近的场景。参见 [大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/).

### 发布到 WORLDs

发布到某个 World 的场景必须通过链接访问。

这些场景与 NAME 代币关联。NAME 代币可以在 [市场](https://decentraland.org/marketplace/names/claim)购买。只要尚未被占用，你可以领取任何你想要的名称。或者，你可以使用一个 [ENS 域名](https://ens.domains) 来创建一个 World。

发布到某个 World 的场景地块限制为 300x300。相同的 [大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 每个地块限制也适用于发布到 LAND 地块的场景，但你可以免费向场景添加更多地块。

如果是 Multi-Scene WORLD，不同的协作者（地址）可以拥有 WORLD 中特定地块的发布权限，该权限由 World Owner（领取了 World NAME 的地址）定义。此外，访问某个 World 可以是 **公开** 或 **私密**。更多关于 [管理 Worlds](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#managing-worlds)

一个 World 可以是 **公开** 或 **私密**

#### 大小限制

你可以上传到 World 的最大文件大小，取决于你使用的是 Decentraland NAME 还是 ENS 域名。

**来自 Decentraland NAME 的 Worlds**

Decentraland NAME 持有者在 Foundation Worlds Content Server 中享有动态存储容量，该容量取决于其钱包持有资产。以下规则决定了这一分配：

* 你拥有的每个 Decentraland NAME 可获得 100 MB 的存储容量（以及一个 World）。
* 你拥有的每个 Decentraland LAND 地块可额外获得 100 MB 的存储容量。
* 钱包中每持有 2,000 MANA，即可额外获得 100 MB 的存储容量。

Foundation Worlds Content Server 中的空间可用于托管用户想要的任意大小的场景，利用其拥有的 Decentraland NAME 以及其持有的各类 Decentraland 资产所共同提供的空间。例如，一位拥有多个 World（由多个 NAME 提供）的用户，若总存储容量为 500 MB，则可以选择部署一个包含 200 MB 场景文件的 World、另一个包含 200 MB 场景文件的 World，以及第三个包含 100 MB 场景文件的 World。或者，他们也可以选择部署一个包含 300 MB 场景文件的 World 和另一个包含 200 MB 场景文件的 World。

你可以在两个地方查看已使用了多少存储额度以及还剩多少：

* 该 **管理** Creator Hub 的 **查看详情** 可查看你的 MANA、LAND 和 NAME 持有量如何累计。
* 该 **Worlds** 选项卡中的 [Builder](https://decentraland.org/builder/worlds)，其中也会显示你的每个 World 正在使用多少空间。

**来自 ENS 域名的 Worlds**

相比之下，由 ENS 域名授予的 Worlds 每个 World 的场景文件最大尺寸固定为 36 MB，且不受用户其他 Decentraland 持有资产的影响。拥有来自 ENS 域名的 World 的用户无法通过购买更多 MANA 或 LAND 来提高其 ENS World 的场景大小上限。

不过，由 ENS 域名所有权授予的 Worlds 是迈向实现 Decentraland 所提供创作自由的完美第一步。

#### 与 LAND 场景的区别

Worlds 相比 LAND 场景有以下几个优势：

* 无需拥有 LAND——可免费向你的场景添加任意数量的地块（最多 300x300）
* 通过链接访问——玩家可直接通过 URL 访问你的 World
* 更多存储容量——尤其适合 Decentraland NAME 持有者
* 访问控制——你可以只允许特定玩家访问你的 World

参见 [Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 了解更多信息，或学习 [发布到 Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/#publishing-to-worlds) 和 [配置 World 设置](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#world-configuration).

## 全局场景

全局场景可以改变 Decentraland 已有的景观，增加多层互动性和玩法。这些场景不局限于只在某些 LAND 地块或某些 Worlds 中运行。玩家无论走到哪里，都可以随身携带它们。

### 智能穿戴物

智能穿戴物与 Wearable 代币关联。它们作为 NFT 出售，并可在 [市场](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

每当玩家穿上关联的穿戴物时，智能穿戴物就会被激活。若玩家脱下该物品，它们就会关闭；或者也可以通过 UI 手动关闭全局场景。此外，玩家所处的场景也能够暂停该场景中任何处于激活状态的智能穿戴物。

* 了解全部关于 [创建穿戴物](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables/).
* 了解 [智能穿戴物](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
