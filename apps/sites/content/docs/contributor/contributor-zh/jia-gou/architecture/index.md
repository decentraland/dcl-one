# 概述

Decentraland 是一个去中心化虚拟世界平台，构建于混合架构之上，结合了区块链所有权、去中心化内容分发和中心化服务，以提升用户体验。

## 架构原则

### 在关键之处实现去中心化

* **所有权** - LAND、穿戴装备和名称是以太坊上的 NFT
* **治理** - DAO 控制关键智能合约和金库
* **内容** - 分布在各个 Catalyst 节点上
* **身份** - 基于钱包的身份验证（无需密码）

### 为性能而集中化

* **实时服务** - WebSocket 通信、语音聊天
* **发现** - 搜索、活动、精选内容
* **社交功能** - 好友、社区、通知
* **优化** - 资产处理、图像生成

## 系统层

### 客户端

玩家通过各种客户端进行交互：

* **Unity Explorer** - 主要桌面客户端（Windows、Mac、Linux）
* **Web Explorer** - 基于浏览器的客户端
* **CLI** - 用于场景创建和部署的开发者工具
* **社区客户端** （移动端、VR 等）- 使用不同渲染技术对参考客户端的替代实现。

所有客户端都通过加密钱包进行身份验证，并遵循相同的协议。

### 发现（Realm Provider）

为客户端提供 realm 描述的单一入口：

* 可用的 Catalyst 节点（位置、容量、健康状态）
* 后端服务端点

可根据地理位置和负载实现智能服务器选择。

### 去中心化内容（Catalyst 网络）

无需许可的 Catalyst 节点网络，用于存储和提供：

* 场景定义和资产
* 头像资料和穿戴装备

每个节点独立验证并存储内容。节点会同步部署，但独立运行。参见 [Catalyst 网络](/docs/contributor/contributor-zh/jia-gou/catalyst/) 详情。

### 集中化功能服务器

后端微服务提供增强功能：

* **服务发现** - Realm 选择和健康监控
* **通信** - WebSocket 集群、语音/视频访问控制
* **内容功能** - 世界、活动、地点、地图数据
* **社交功能** - 好友、社区、通知
* **平台功能** - 成就、奖励、虚拟积分
* **资产处理** - 优化、缩略图、翻译

查看 [后端服务](/docs/contributor/contributor-zh/jia-gou/services/) 以获取完整的服务文档。

### 基础设施

支持系统和第三方服务：

* **消息代理** - 用于服务间通信的 NATS
* **媒体服务器** - 用于语音和视频的 LiveKit
* **区块链索引器** - 用于 NFT 数据的快速查询
* **电子邮件服务** - 用于通知的 SendGrid

查看 [基础设施](/docs/contributor/contributor-zh/jia-gou/infrastructure/) 详情。

### 区块链

以太坊和 Polygon 上的智能合约管理：

* **所有权** - LAND、穿戴装备、名称、地块
* **市场** - 交易和版税
* **DAO** - 治理和金库
* **货币** - MANA 代币

## 安全模型

### 身份与认证

* **钱包签名** - 无需密码，通过以太坊签名进行身份验证
* **认证服务器** - 在签名验证后签发 JWT 令牌
* **签名获取** - API 使用钱包签名来验证请求

### 内容验证

* **签名验证** - 所有部署都由钱包签名
* **指针所有权** - 只有 LAND 拥有者才能向坐标部署
* **大小限制** - 内容大小受地块数量限制

### 访问控制

* **限时令牌** - JWT 和 LiveKit 令牌会过期
* **速率限制** - 按 IP/用户设置 API 速率限制
* **DDoS 防护** - 分布式 Catalyst 网络

## 可扩展性方案

### 水平扩展

* **Catalyst 节点** - 随着内容增长添加节点
* **后端服务** - 无状态微服务可独立扩展
* **岛屿** - 通信按邻近关系划分为不同组

### 优化

* **资产包** - 平台优化的资产（Unity bundles）
* **CDN** - 静态资源通过 CDN 提供
* **缓存** - 在所有层进行激进缓存
* **索引器** - 区块链数据缓存于高速数据库中

## 下一步

深入了解特定的架构领域：

* [**后端服务**](/docs/contributor/contributor-zh/jia-gou/services/) - 详细的服务规范和集成
* [**Catalyst 网络**](/docs/contributor/contributor-zh/jia-gou/catalyst/) - 内容分发和部署
* [**基础设施**](/docs/contributor/contributor-zh/jia-gou/infrastructure/) - 支持系统和第三方服务

## 其他资源

* [API 参考](https://github.com/decentraland/docs/tree/main/apis/README.md) - 完整的 API 文档
* [贡献者指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/contributor-guides/) - 开发标准
* [GitHub 组织](https://github.com/decentraland) - 源代码仓库


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/jia-gou/architecture.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
