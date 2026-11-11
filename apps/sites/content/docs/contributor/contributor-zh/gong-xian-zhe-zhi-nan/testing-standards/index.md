# 测试标准

本节描述了在 Decentraland 各项目的不同服务和 UI 中进行测试的方式。

本文档中的关键词“必须”、“禁止”、“要求”、“应”、“不得”、“应该”、“不应该”、“推荐”、“可以”和“可选”应按照以下所述进行解释： [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## 概览

测试是我们开发流程中的关键部分。我们在所有项目中都保持对测试覆盖率和质量的高标准。本文档为以下内容提供了全面指南：

* [**编写测试**](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/) - 编写可维护、清晰测试的一般原则和模式
* [**测试 UI**](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-uis/) - 测试 React 组件、Redux saga、reducer、selector 和 action creator
* [**测试服务（知名组件）**](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc/) - 测试基于 well-known-component 架构构建的模块化服务
* [**测试服务（已弃用）**](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-deprecated/) - 测试服务的旧方法（供参考）
* [**端到端测试**](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-e2e/) - 使用 Cypress 进行 E2E 测试（进行中）

## 测试理念

我们的测试方法强调：

* **清晰性** - 测试应具备自解释性且易于理解
* **可维护性** - 当需求变化时，测试应易于更新
* **隔离性** - 每个测试都应独立运行，不影响其他测试
* **覆盖率** - 应测试所有关键路径和边界情况
* **值** - 测试应提供代码按预期运行的信心

## 快速链接

* [如何使用……描述上下文 `describe`](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#describing-and-building-context)
* [如何使用……编写期望 `其`](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#describing-expectations-and-executing-code)
* [测试什么以及何时测试](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#what-to-test)
* [何时以及如何模拟](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#when-to-mock)

## 相关标准

* [依赖管理](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/dependency-management/) - 管理 npm 依赖和 peerDependencies
* [UI 标准](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - Redux 和 RTK Query 模式
* [API 文档](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/api-documentation/) - OpenAPI 文档标准


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
