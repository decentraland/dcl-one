> 企鹅滑行玩法，带有定时横扫动作，可根据与目标的距离改变减速和计分。

# 滑行企鹅

将企鹅滑向目标，并使用定时滑动来影响它减速的速度。

部署说明：当前未部署。

* [在 Decentraland 上游玩](https://decentraland.org/jump?position=-140%2C101)

## 项目信息

**维护者：** 社区 · ToxSam

[查看上游源码](https://github.com/ToxSam/SlidePenguin)

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

### 将滑动转变为临时移动修饰器

`handleSwipe` 为减速降低添加冷却，并记录该效果何时失效。 `penguinMovementSystem` 在该时间窗口结束后恢复基础减速度，更新速度，并使用帧时间推进变换。

**项目背景：** 遵循项目的 `PenguinMovement` 和 `SwipeBox` 组件和调优常量，用于理解输入与运动之间的联系。

**源码参考：**

* [`handleSwipe`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L519-L598)
* [`penguinMovementSystem`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L864-L913)

### 为物体落点与目标的接近程度打分

`calculateDistance2D` 和 `calculateScore` 忽略高度，将水平距离相对于最大距离归一化，并映射为一个四舍五入的分数，上限为 `MAX_SCORE`.

**项目背景：** 这个小模块是理解接近度评分的一个有用起点；其输入是最终的企鹅位置和目标位置。

**源码参考：**

* [`calculateDistance2D` 和 `calculateScore`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/scoring.ts#L5-L39)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/slide-penguin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
