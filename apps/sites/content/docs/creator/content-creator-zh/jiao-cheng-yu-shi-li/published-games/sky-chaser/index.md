> 具有持久化排行榜、复用投射物以及从回合状态种子生成拾取物位置的多人接包回合。

# 追云者

![Sky Chaser 预览](/docs/_remote/3033913f0028e180a375c5bf.webp)

一款多人游戏，玩家在每回合中竞争追逐逃离的包裹。场景包含周榜和总榜。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=skychaser.dcl.eth)

## 项目信息

**维护者：** 社区 · stom66

[查看上游源码](https://github.com/stom66/dcl-sky-chaser)

**许可证：** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-sky-chaser/blob/main/LICENSE.md)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* 多人服务器
* 通过同步的 Mulberry32 随机数生成器实现同步游戏玩法
* 用于排行榜的存储 API
* 针对加速器/蹦床/爆炸的玩家移动微调和物理力
* 粒子系统（加速轨迹、爆炸、拾取物）

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 在游戏过程中重用投射物实体

`fireProjectile` 和 `getIdleProjectile` 重用未激活的投射物；当对象池耗尽时创建新的投射物，并且只更新处于激活状态的投射物。

**项目背景：** 遵循项目的 `投射物` 用于移动、碰撞响应和回合结束清理的类和事件处理器。

**源码参考：**

* [`fireProjectile` 和 `getIdleProjectile`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/projectileManager.ts#L76-L124)

### 根据回合状态为拾取物位置设定种子

`onGameStart` 和 `getRandomPosition` 使用回合开始时间初始化项目的随机生成器，然后采样拾取物的距离和高度。

**项目背景：** 同时读取拾取物配置和组件存储：移动限制以及后续的拾取物活动会影响随机调用序列，因此仅靠共享种子并不能保证每个客户端上的布局完全一致。

**源码参考：**

* [`onGameStart` 和 `getRandomPosition`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/spawners/pickupSpawner.ts#L115-L164)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/sky-chaser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
