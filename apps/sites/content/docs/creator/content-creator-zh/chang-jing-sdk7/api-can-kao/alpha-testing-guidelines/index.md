> 关于如何尝试 SDK7 并提供反馈的指南

# 测试与反馈

## 文档

请在以下位置查找 SDK7 的完整文档： **SDK7** 文档中的该部分。

## 使用沙盒

如需轻松试用 SDK7，请访问 [沙盒](https://playground.decentraland.org/)。在屏幕左侧编写 SDK7 代码，并在右侧查看生成的场景。当你更改代码时，场景会自动更新。

如果你遇到任何问题，请使用 **分享** 按钮复制当前场景设置的 URL，并 [报告问题](#reporting-issues) ，同时附上该代码链接。

## 使用 SDK 7

先安装 [创作者中心](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/).


> [!WARNING]
> **📔 注意**：请注意，使用 SDK7 的场景没有安装 `decentraland-ecs` 包。取而代之的是，它们安装了 `@dcl/sdk` 包。


## 在生产环境中

你可以通过 Creator Hub 中 Scene Editor 的 **发布** 按钮部署使用 SDK7 的场景。

## 示例场景

你可以在这里找到更多使用 SDK7 编写的示例场景：

* [场景示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)

## 报告问题

如果你遇到问题，请查看 [报告错误](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/alpha-testing-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
