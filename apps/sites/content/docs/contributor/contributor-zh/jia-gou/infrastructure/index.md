# 基础设施

本页面介绍为 Decentraland 提供支持的基础设施和外部服务。

## 外部服务（SaaS）

### LiveKit

**目的**：高质量的语音和视频媒体服务器

[LiveKit](https://livekit.io/) 提供：

* WebRTC 媒体路由
* 语音聊天
* 视频会议
* 屏幕共享
* 空间音频（开发中）

**访问控制**: [通信守门人](https://github.com/decentraland/docs/tree/main/apis/apis/comms-gatekeeper/README.md) 生成限时令牌

**由以下使用**:

* Archipelago Workers - 默认语音
* Worlds - 每个世界的语音频道
* Events - 事件专用频道

### SendGrid

**目的**：电子邮件投递服务

发送通知邮件：

* 好友请求
* 活动提醒
* 系统通知

**由以下触发**: [通知系统](https://github.com/decentraland/docs/tree/main/apis/apis/notifications-workers/README.md)

***

## 区块链索引器

多个索引服务提供快速的区块链查询：

### Squids

**目的**：自定义区块链索引器

[Subsquid](https://www.sqd.ai/) 基础的索引器：

* 自定义数据模型
* 快速同步时间
* GraphQL API

**索引**:

* 地块（LAND NFT）
* 可穿戴设备和收藏品
* 市场活动
* ENS 名称

### Satsuma

**目的**：托管的 GraphQL 提供商

托管的 subgraph 托管：

* 高可用性
* 自动扩缩容
* 包含监控

### The Graph

**目的**：去中心化索引协议

[The Graph](https://thegraph.com/) 网络：

* 去中心化索引器
* 以 GRT 计费的查询费用
* 由社区运营

***

## 消息代理

### NATS

**目的**：发布/订阅消息路由

[NATS](https://nats.io/) 提供：

* 实时消息传递
* 发布/订阅模式
* 请求/响应模式
* 支持集群

**由以下使用**:

| 服务          | 目的   |
| ----------- | ---- |
| Archipelago | 位置更新 |
| 社交服务        | 好友状态 |
| 世界          | 世界事件 |

***

## 媒体处理

### 图像转换服务（Metamorph）

**目的**：图像格式转换和优化

在以下格式之间转换图像：

* WebP 优化
* 缩略图生成
* 格式转换

**GitHub**: [metamorph](https://github.com/decentraland/metamorph)

### 个人资料图片系统

**目的**：头像缩略图生成

生成头像预览图：

* 面部特写
* 全身照
* 多种分辨率

**组件**:

* **个人资料图片消费者** - 渲染头像
* **个人资料生产者** - 监听头像变更

***

## 资产处理工作器

### 资产包转换器

**目的**：平台特定的资产优化

将已部署的资产转换为适用于所有平台的优化包。

**将包注册到**: [资产包注册表](https://github.com/decentraland/docs/tree/main/apis/apis/asset-bundle-registry/README.md)

### LOD 生成器

**目的**：细节层级变体

生成 3D 模型的低多边形版本，以提升性能。

***

## 奖励系统

### 奖励 API

**目的**：NFT 奖励分发

为以下操作授予 NFT 奖励：

* 完成引导
* 参与活动
* 推荐

***

## 下一步

* 回顾 [后端服务](/docs/contributor/contributor-zh/jia-gou/services/) 用于服务规范
* 检查 [Catalyst 网络](/docs/contributor/contributor-zh/jia-gou/catalyst/) 用于内容基础设施
* 查看 [架构概览](/docs/contributor/contributor-zh/jia-gou/architecture/) 用于完整视图


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/jia-gou/infrastructure.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
