> 具有武器进阶、复用奖励文本以及敌人在其他僵尸和障碍物周围本地寻路的僵尸生存游戏。

# 僵尸潮

![Dead Surge 预览](/docs/_remote/fbb6dfc582416446805d17d3.webp)

带有战斗和武器成长的僵尸生存游戏。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=deadsurge.dcl.eth)

## 项目信息

**维护者：** Decentraland DAO

[查看源代码](https://github.com/dcl-regenesislabs/dead-surge)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 为浮动奖励文本复用实体

`spawnZcRewardTextAtPosition` 及其对象池初始化器会循环使用一组固定的文本实体。该 `rewardTextSystem` 让它们上升、面向摄像机，并在淡出后隐藏，以便复用。

**项目背景：** 阅读场景的 `RewardTextComponent` 以及游戏时间更新，使用这些函数来了解它们的生命周期跟踪。

**源码参考：**

* [`spawnZcRewardTextAtPosition` 以及其对象池初始化器](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L592-L640)
* [`rewardTextSystem`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L815-L850)

### 引导敌人彼此绕行并避开简单障碍

`getZombieSeparationDirection` 将附近僵尸的排斥力合并。 `resolveZombieMovePosition` 会针对圆形砖块障碍测试期望方向、备用方向和各个移动轴。

**项目背景：** 这展示了局部转向；其行为取决于场景的移动条目、障碍物位置和半径常量。

**源码参考：**

* [`getZombieSeparationDirection`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L868-L896)
* [`resolveZombieMovePosition`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L898-L939)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/dead-surge.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
