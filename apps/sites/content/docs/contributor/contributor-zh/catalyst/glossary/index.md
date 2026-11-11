# 术语表

#### Catalyst

[Catalyst](https://github.com/decentraland/catalyst) 是赋予……的名称 **Decentraland 服务包**，每个 Catalyst [节点](https://decentraland.github.io/catalyst-monitor) 公开暴露一组服务，这些服务作为平台的支柱，同时还暴露一个公开的 [API](https://decentraland.github.io/catalyst-api-specs/).

#### 引导

当 Catalyst 启动时，节点将处于 `Bootstrapping` 状态，在从其他 Catalyst 获取内容时，直到追上其他节点。然后它会转入下一个状态 `Syncing` ，在该状态下它已准备好提供内容。

#### Catalyst 客户端

[库](https://github.com/decentraland/catalyst-client) ，可用于与 Catalyst 服务器交互。它既允许获取数据，也允许向指定服务器部署新实体。

#### 实体

数据类型，由 [模式](https://github.com/decentraland/common-schemas)定义，由于体积原因无法存储在区块链中。当前支持的某些 [支持的类型](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13) 如下：

* [可穿戴物](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/wearable/wearable.ts)
* [表情动作](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/emote/emote.ts)
* [场景](https://github.com/decentraland/common-schemas/blob/main/src/platform/scene/scene.ts)
* [资料](https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts)
* [商店](https://github.com/decentraland/common-schemas/blob/main/src/dapps/store.ts)
* [服装搭配](https://github.com/decentraland/common-schemas/blob/main/src/platform/outfits/outfits.ts)

#### 部署

在内容服务器上创建并通过唯一 [EntityId](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L31)标识的实体实例。部署可以是任意 [实体类型](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13).

#### 部署失败

部署到内容服务器的实体实例，未能通过 [内容验证](https://adr.decentraland.org/adr/ADR-51)。验证是在 [内容验证器](https://github.com/decentraland/content-validator) 库。

#### 活跃实体

当前部署在某个指针上的实体，因此所有活跃实体都代表 Decentraland 的实时内容：场景、个人资料、可穿戴物等。

#### 指针

指针是部署所属的 ID 或位置。其格式会根据 `实体类型`而有所不同，例如：

* **可穿戴物品**：使用 URN 作为指针 `urn:decentraland:ethereum:collections-v1:dg_atari_dillon_francis:gerald_patchwork_knit_sweater`
* **场景**：使用地块或地块数组作为指针，例如 `--10,20` 或 `[(0,0), (0,1), (0,-1)]`
* **个人资料**：使用钱包地址作为指针 `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75`
* **商店**：使用 URN 作为指针 `urn:decentraland:off-chain:marketplace-stores:{wallet}`
* **服装搭配**：使用以……为后缀的钱包地址作为指针 `:outfits`, `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75:outfits`

#### URN

[URN](https://en.wikipedia.org/wiki/Uniform_Resource_Name) 代表统一资源名称。它们用于作为元宇宙项目中资产的全局唯一标识符。

* URN 解析器：<https://github.com/decentraland/urn-resolver>
* URN 命名空间：<https://github.com/common-metaverse/urn-namespaces>

#### 内容服务器

该 [内容服务器](https://github.com/decentraland/catalyst/tree/main/content) 是 Catalyst 节点的一部分，负责管理实体部署的去中心化存储。目前，Content Server 中存储的所有内容都会与 DAO 其余的 Catalyst 同步。

#### Lambda

该 [Lambda](https://github.com/decentraland/catalyst/tree/main/lambdas) Service 是 Catalyst 节点的一部分，作为 Content Servers 与区块链之间的协调层，负责清理内容、验证所有权，并帮助 Catalyst 客户端检索信息。

#### 同步

Content Servers 提供在其上部署内容的能力。它们还确保 DAO 中其他 Catalyst 上部署的任何内容也同样可在此处获得。解决这一问题称为 **同步**.

#### 内容快照

某个特定类型在一个 [内容服务器](#content-server)中的所有活跃实体的集合。此内容快照用于在 Catalyst 节点之间同步内容。

#### 领域

当连接到 Decentraland 时，你只能与连接到同一领域的 Peer 交互。由于友好名称领域使用了希腊神祇的名字，例如 Hela 或 Loki。每个领域由一个单一的 [Catalyst 节点](https://decentraland.github.io/catalyst-monitor/).

#### Peer

连接到 Decentraland 的用户。

#### Lighthouse

该 [Lighthouse](https://github.com/decentraland/lighthouse) 是用于引用托管在 Catalyst 节点上的 Communications Service 旧版本的名称，用于在 P2P 网络中对用户进行分组。其主要职责是确定哪些是 P2P 连接的候选者，并进行 WebRTC 信令以建立连接。其中大部分逻辑通过两个外部组件完成：PeerJS Server（连接 [WebRTC](https://webrtc.org/) 对等节点）和 Archipelago（接收用户位置并将他们分组到岛屿中）。

#### 面向前端的后端（即 BFF）

该 [BFF](https://github.com/decentraland/explorer-bff) Service 的创建是为了解决客户端需求，并在不破坏现有 API 的情况下加快新功能开发。在 Catalyst 通信服务的上下文中，它将负责管理 P2P 信令，从而取代 Lighthouse。

#### Catalyst 所有者

一个 [Catalyst 所有者](https://github.com/decentraland/catalyst-owner) 是运行 Catalyst 节点的个人或组织。要将一个 Catalyst 节点加入 DAO 网络，必须提交一个 [提案](https://governance.decentraland.org/?type=catalyst) ，并且 DAO 节点代表受信任的 Decentraland 网络。

#### Archipelago 服务

该 [Archipelago 服务](https://github.com/decentraland/archipelago-service) 此前是 Lighthouse 使用的一个库。此服务将承担与该库相同的职责：将对等节点分组为集群，以便它们能够高效通信。每个集群称为一个岛屿。另一方面，该服务还需要能够根据可用的通信传输方式，并遵循一组 [Catalyst 所有者](https://github.com/decentraland/catalyst-owner) 定义的规则来平衡各个岛屿，例如在 Casino 场景中为某个岛屿使用 LiveKit，而在 Genesis Plaza 中使用 P2P。

#### 岛屿

已连接对等节点的集群。

#### 通信传输

传输是用于在连接到某个岛屿的对等节点之间共享更新的技术。

#### P2P 传输

使用 [WebRTC](https://webrtc.org/) 对等节点之间的连接，无需额外基础设施即可扩展，因为负载在客户端侧，服务器只负责信令。随着向岛屿添加更多对等节点，延迟会增加。客户端可建立的 WebRTC 连接数量是有限的，因此为了拥有更大的岛屿，需要在对等节点之间建立更多间接连接（跳跃）。

#### LiveKit 传输

[LiveKit](https://livekit.io/) 是一个开源项目，提供可扩展的多用户 WebRTC 会议能力。它不使用 P2P 网络，而是将对等节点连接到一个选择性转发单元（SFU），负责管理消息转发和通信的不同质量方面。SFU 是为了在指定场景中的人群之间提供高性能/高质量通信而需要增加的基础设施。

#### WebSocket 传输

该 [WebSocket 传输](https://github.com/decentraland/ws-room-service) 比 LiveKit 更轻量，它也需要一个服务器来通过 WebSocket 房间管理消息转发。房间相当于岛屿。与 P2P 岛屿相比，此传输提供更低的延迟和更好的性能，但也需要额外的基础设施。

#### NATS

[NATS](https://nats.io/) 是一种消息代理，使服务之间能够进行数据交换和通信。在 Catalyst 服务器的上下文中，它用于通信某些内部服务。

#### DAO

DAO 代表“Decentralized Autonomous Organization（去中心化自治组织）”。Decentraland DAO 拥有构成 Decentraland 最重要的智能合约和资产——LAND 合约、Estates 合约、可穿戴物、内容服务器和 Marketplace。它还拥有大量 MANA 资金，这使其能够真正实现自治，并为整个 Decentraland 的各种运营和计划提供补贴。

<https://dao.decentraland.org/en/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/catalyst/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
