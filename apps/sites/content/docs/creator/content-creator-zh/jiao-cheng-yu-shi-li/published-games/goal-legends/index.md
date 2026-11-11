> 由服务器运行比赛、持久化排行榜、阶段驱动摄像机和同步球动画的多人点球大战。

# 进球传奇

![Goal Legends 预览](/docs/_remote/aa680097357467a542aed322.webp)

一款多人点球大战游戏。与另一位玩家或 GL-Bot 对决：选择左路、中路或右路来进球或扑救。每一轮都会以电视转播风格进行回放结束，观众会实时做出反应。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=goallegends.dcl.eth)

## 项目信息

**维护者：** 社区 · carlosmu

[查看上游源码](https://github.com/carlosmu/goal-legends-arena)

**许可证：** [Apache-2.0 声明](https://github.com/carlosmu/goal-legends-arena/blob/master/LICENCE)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 主要功能

* **多人服务器** 负责运行比赛
* **存储** 用于每日/历史 PvP/PvE 排行榜，以及每位玩家的国家旗帜。
* **场景** **表情动作**
* 与动画器同步的球道具，用于射门、扑救、进球和救球。
* **虚拟摄像机** 用于所有人都能看到的游戏视图和获胜过场动画。
* **头像形状** 会对每次进球和扑救作出反应的观众 NPC。
* **粒子** 用于烟花效果
* 一个带有灯光和 **`Gltf 节点修饰器`** 会把你带到另一个世界。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 根据比赛阶段切换摄像机

`initGameplayCamera` 从客户端比赛快照中选择游戏摄像机、获胜摄像机或标准摄像机。获胜摄像机会绕着一个枢轴旋转，而观众在比赛中可以选择自由摄像机。

**项目背景：** 请将此与 `clientSnapshot`、 `GameState` 枚举，以及命名的场景摄像机枢轴一起查看。

**源码参考：**

* [`initGameplayCamera`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/gameplayCamera.ts#L109-L165)

### 将道具动画与游戏状态协调起来

`tickAnimationManager` 在阶段变化时启动球的片段，根据射门方向选择进球/扑救动画，并在切换片段时短暂隐藏该道具。此路径专用于机器人踢球者；人类踢球者会在表情动作中携带球。

**项目背景：** 这取决于场景中的球模型和命名动画片段。

**源码参考：**

* [`tickAnimationManager`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/animationManager.ts#L78-L114)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/goal-legends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
