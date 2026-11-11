> 一款社交农场游戏，具有定时作物生长阶段、肥料加成，以及由一个实体层级构建的重复农场。

# 温馨农场

![Cozy Farm 预览](/docs/_remote/f005f5c4d6dce02d9535e094.webp)

一款社交农场游戏，专注于种植作物、扩建农场和结识其他玩家。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=cozyfarm.dcl.eth)

## 项目信息

**维护者：** Decentraland DAO

[查看源代码](https://github.com/dcl-regenesislabs/cozy-farm)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将经过的时间转化为作物生长阶段

`growthSystem` 根据种植时间戳推导进度，针对教程作物和肥料调整持续时间，并且仅在阶段变化时切换模型。

**项目背景：** 配合以下内容一起查看 `PlotState`, `CROP_DATA`，以及其中的模型辅助函数 `game/actions` 以了解模拟状态如何驱动视觉反馈。

**源码参考：**

* [`growthSystem`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/growthSystem.ts#L62-L142)

### 从一个实体层级构建多个农场

`cloneEntityTree` 递归重建模板的子对象，保留局部变换，复制选定的渲染组件，并按名称记录每个克隆。这说明重复区域如何共享同一套预先制作的布局。

**项目背景：** 该辅助函数需要场景的子对象映射，并复制一份明确的组件列表，因此新的组件类型需要专门处理。

**源码参考：**

* [`cloneEntityTree`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/farmInstances.ts#L127-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/cozy-farm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
