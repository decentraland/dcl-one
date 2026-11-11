> 三车道竞速，配有循环复用的场景、基于距离的记录幽灵和持久化排行榜数据。

# 竞速之巅

![Prime Drive 预览](/docs/_remote/bba6ea4b5469647296b97b46.webp)

一款三车道自行车赛车游戏，玩家需要躲避障碍物、收集金币，并在 10 公里的赛道上追逐赛道纪录幽灵。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=primedrive.dcl.eth)

## 项目信息

**维护者：** 社区 · carlosmu

[查看上游源码](https://github.com/carlosmu/dcl-prime-drive)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。


> [!WARNING]
> **社区维护代码——使用前请审查**
> 
> 该社区项目由 Decentraland 基金会和 DAO GitHub 组织之外的团队维护。其源代码和线上部署可能会独立变更。
> 
> 收录到本目录并不意味着已通过安全审计，也不保证安全。在运行或重新使用前，请审查代码、依赖项和安装脚本。
> 
> [报告目录问题](https://github.com/decentraland/docs/issues/new)。如怀疑存在漏洞，请遵循上游项目的安全策略。不要在公开 issue 中包含利用细节或凭据。


## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 使用循环场景创建一场长距离比赛

`setWorldOffset` 和 `scrollTrack` 将共享的世界根节点横向移动以进行变道，并在路面装饰经过玩家后将其循环放回前方。自行车保持原地不动，而场景提供运动感。

**项目背景：** 读取环形生成器和共享的 `赛道`/`生成` 设置，以了解间距和循环距离。

**源码参考：**

* [`setWorldOffset` 和 `scrollTrack`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/track.ts#L43-L92)

### 相对于玩家显示纪录幽灵

`updateGhosts` 根据距离差放置幽灵自行车，将视野范围外的幽灵隐藏，并平滑地将它们移动到远离玩家的车道中。

**项目背景：** 此模块读取 `GhostRunView` 数据；它显示提供的进度，本身不记录重放。

**源码参考：**

* [`updateGhosts`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/ghost.ts#L129-L183)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/prime-drive.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
