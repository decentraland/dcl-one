# MAIN Realm

Decentraland 拥有的领域数量与提供通信服务的 Catalyst 数量相同，但其中有一个独特的领域称为 MAIN，它存在于 Catalysts 网络之外。MAIN 领域专门为简化用户连接流程而设计，使用户更容易进行互动和见面。它由一组能够替代 Catalyst 节点通信服务的服务构成。该领域是用户登录 Decentraland 后的默认目的地。

<img src="/docs/_remote/d01dbf731e2a22a53eeb9e6a.webp" alt="" width="800">

该 [领域提供者](https://github.com/decentraland/realm-provider) 服务公开一个 [/about](https://realm-provider.decentraland.org/main/about) 包含领域描述的端点，遵循 [ADR-110：领域描述](https://adr.decentraland.org/adr/ADR-110)。这提供了建立与内容管理网络连接所需的信息，该网络是 Catalyst 节点的服务之一，同时也提供了 MAIN 领域通信服务的连接详情。

MAIN 通信服务可通过以下方式访问： [WebSocket 连接器](https://github.com/decentraland/archipelago-workers)。这个轻量级组件可与其余后端组件交互，并且可以通过增加实例轻松扩展，以适应不断增长的用户负载，高效管理更多活动连接。通过 WebSocket 连接，客户端会获取与 LiveKit Cloud 建立连接所需的数据，以及 `islandId` 由 Archipelago Service 分配。Archipelago Service 在 MAIN 领域和 Catalyst 节点通信服务中承担相同职责。它有效管理用户位置的处理及其后续分配到不同的岛屿，每个岛屿对应一个 LiveKit 房间，从而根据用户在虚拟世界中的空间位置高效组织用户群组。如需进一步了解 Archipelago Service，可参考 [Archipelago](/docs/contributor/contributor-zh/tong-xin/archipelago/) 页面。

[NATS](https://nats.io/) 作为一个广播系统，促进各项服务之间的无缝通信。它可以支持多个 WebSocket Connector 实例与同一个 Archipelago Service 通信。此外，它还使 Stats 服务能够从 Archipelago 收集用户统计信息，如位置和岛屿分组，从而使这些数据可供外部使用。

Archipelago 统计服务与 Catalyst 节点中的 Archipelago 服务共享相同的 API 实现。你可以通过以下链接查看该 API： [Catalyst API 规范](https://decentraland.github.io/catalyst-api-specs/#tag/Archipelago)。该服务可通过以下 URL 访问 `https://archipelago-stats.decentraland.org/` 可用端点位于 `/comms/` ，如 API 规范所述，例如： `https://archipelago-stats.decentraland.org/comms/peers`.

该 [LiveKit](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/livekit/) 云平台负责通过 WebRTC 交换用户信息这一复杂任务，并且是大多数 Catalyst 节点为实现同一目的而共享的主要云平台。

从 MAIN 领域得出的基本概念是，不同用户可以灵活地连接到任意 Catalyst 节点进行内容管理（下载场景、穿戴物、表情动作或发送个人资料更新），但无论选择了哪个特定的内容管理节点，与通信服务的连接 **可以** 保持相同。因此，即使用户使用不同的 Catalyst 节点作为内容服务器，但他们处于相近的场景坐标中，也能够相互互动。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/main-realm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
