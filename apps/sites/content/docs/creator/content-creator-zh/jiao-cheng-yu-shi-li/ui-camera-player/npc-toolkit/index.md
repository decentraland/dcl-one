> 适用于 SDK7 场景的可复用 NPC 行为和对话界面。

# NPC 工具包

在 SDK7 场景中使用可复用的 NPC 行为和对话 UI。

## 项目信息

**维护者：** Decentraland

[查看源代码](https://github.com/decentraland-scenes/dcl-npc-toolkit)

**许可证：** [Apache-2.0](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/main/LICENSE)。在重复使用前请检查上游条款；代码和资源可能适用不同条款。

## 浏览代码

这些链接指向特定的源码快照，因此即使上游持续开发，相关说明仍然有用。它们是学习参考，而非安全评估。

### 将展示对话与推进对话分开

`handleDialogClick`, `rushText`，以及 `confirmText` 如果文本仍在逐字显示，先显示剩余字符。后续输入会推进脚本或选择数字对话分支，并附带游戏动作回调。

**项目背景：** 请结合工具包的 `对话` 数据和 `npcDialogComponent` 状态一并阅读，以了解 UI 输入如何驱动一段对话。

**源码参考：**

* [`handleDialogClick`, `rushText`，以及 `confirmText`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/dialog.ts#L489-L549)

### 将接近与指针输入连接到 NPC 交互

`addClickReactions` 和 `addTriggerArea` 配置指针回调和一个子球形触发器，支持仅点击激活以及玩家走开后继续对话的设置。 `激活` 和 `endInteraction` 协调当前激活的 NPC、冷却时间、对话以及可选的面向玩家行为。

**项目背景：** 这些函数依赖于工具包的回调映射和跟踪系统。

**源码参考：**

* [`addClickReactions` 和 `addTriggerArea`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L410-L531)
* [`激活` 和 `endInteraction`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L769-L823)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/ui-camera-player/npc-toolkit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
