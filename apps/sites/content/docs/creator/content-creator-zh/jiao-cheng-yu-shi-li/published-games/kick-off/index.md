> 一个世界杯预测场景，包含蓄力踢球、即时本地运动反馈和响应式赛程面板。

# 开球

![开球预览](/docs/_remote/e381e39b01875ac25f3f33f9.webp)

一个季节性的世界杯预测游戏。

* [在 Decentraland 上游玩](https://decentraland.org/jump?realm=kickoff.dcl.eth)

## 项目信息

**维护者：** Decentraland DAO

[查看源代码](https://github.com/dcl-regenesislabs/kickoff-2026)

**许可证：** 本目录中未标识。在重新使用前，请先查阅上游仓库和维护者以了解适用条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将蓄力踢球与即时视觉反馈连接起来

蓄力-释放循环结合了键盘和移动端输入，增大并重新着色蓄力条，发送踢球消息，并启动本地球的运动。 `乐观释放` 初始化该运动。

**项目背景：** 结合项目的 `prodeNet` 房间和球消息处理程序，来理解输入、反馈和多人消息之间的关系。

**源码参考：**

* [蓄力-释放循环](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L491-L536)
* [`乐观释放`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L155-L178)

### 从参考画布缩放 UI

`布局缩放` 读取 `UiCanvasInformation`，将两个尺寸与 1920×1080 的设计空间进行比较，对结果进行限制，并应用额外的移动端系数。

**项目背景：** 这是一个共享布局辅助函数的紧凑示例；这些限制和移动端系数是为该项目的面板调校的。

**源码参考：**

* [`布局缩放`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/schedule/responsive.ts#L1-L21)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/published-games/kick-off.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
