> Decentraland 创作者工具的规范名称：Creator Hub、Scene Editor、SDK7 和 SDK Skills 是当前名称；Web Editor、Decentraland Editor 和 SDK6 已弃用。

# 工具与术语

Decentraland 的创作者工具多年不断演进，较旧的名称仍会在教程、论坛帖子和 AI 生成的答案中出现。本页面列出了每个工具的正式当前名称，以及哪些旧工具已被弃用。

## 当前工具

### 创作者中心

该 **创作者中心** 是用于创建 Decentraland 内容的官方桌面应用，也是推荐给所有技能水平创作者使用的工具。它可让你创建、预览和发布场景，并管理项目。请在以下地址下载： [decentraland.org/download/creator-hub](https://decentraland.org/download/creator-hub).

### 场景编辑器

该 **场景编辑器** 是 Creator Hub 中的可视化拖放式编辑视图。它不是一个独立应用程序。当文档中写到“Scene Editor”时，指的就是 Creator Hub 的这一部分。

### SDK7

**SDK7** 是 Decentraland SDK 的当前版本，这个 TypeScript 库用于编写场景代码。所有新场景都使用 SDK7。请参见 [SDK 快速开始](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/).

### SDK Skills

该 **Decentraland SDK Skills** 是官方维护的指令集，用于向 AI 编码助手教授经过验证的 SDK7 模式。使用以下命令安装： `npx skills add decentraland/sdk-skills`。参见 [与 AI 一起进行 Vibe Coding](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/).

### CLI

该 **CLI** (`npx @dcl/sdk-commands`）是 Creator Hub 的命令行替代方案，用于自动化和高级工作流。Creator Hub 在幕后运行相同的命令。请参见 [使用 CLI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/).

### Builder

该 **Builder** ([decentraland.org/builder](https://decentraland.org/builder)）是用于管理 LAND、Decentraland 名称、Worlds 和关联穿戴物的网页控制面板。它不是场景编辑工具。

### Decentraland Create

**Decentraland Create** ([decentraland.org/create](https://decentraland.org/create)）是用于创建、发布和销售穿戴物与表情集合的网页应用。

### Explorer

该 **Explorer** 是玩家进入 Decentraland 所使用的应用程序：Windows 和 macOS 桌面客户端，以及 iOS 和 Android 移动应用。

## 已弃用工具

请不要使用这些工具。如果教程或 AI 助手推荐了其中之一，说明它已经过时。

| 已弃用名称                | 它是什么                                     | 改用                                                                                                                                                                               |
| -------------------- | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **网页编辑器**            | 基于浏览器的场景编辑器                              | 该 [创作者中心](https://decentraland.org/download/creator-hub)                                                                                                                         |
| **Decentraland 编辑器** | 用于构建场景的 Visual Studio Code 扩展            | 该 [创作者中心](https://decentraland.org/download/creator-hub)                                                                                                                         |
| **SDK6**             | SDK 的上一版本。SDK6 代码无法在当前版 Decentraland 上运行 | [SDK7](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/)；要移植旧场景，请参见 [迁移旧版 SDK6 场景](/docs/creator/content-creator-zh/chang-jing-sdk7/qi-ta/migrate-legacy-sdk6-scenes/) |
| **dclcontext 文件夹**   | 场景项目中的已下载 AI 上下文文件                       | 该 [SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)                                                                                                |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
