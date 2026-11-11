# 概述

本节包含 Decentraland 服务的贡献者指南和最佳实践。这些指南可确保所有项目的一致性、质量和可维护性。

## 章节

* [API 文档](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/api-documentation/) - 用于记录和发布服务 API 的 OpenAPI 标准
* [测试标准](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/) - 针对不同类型应用和服务编写测试的综合指南
* [常驻组件](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md) - 使用 WKC 架构构建模块化、可测试服务的标准
* [UI 标准](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - 用于构建现代 React 应用的 Redux Toolkit 和 RTK Query 模式
* [Web UI 标准](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md) - 使用 UI2、Material UI 和 Decentraland 设计系统的组件库标准

## 关于这些标准

这些标准中的关键词“必须”、“禁止”、“要求”、“应”、“不应”、“应该”、“不应该”、“建议”、“可以”和“可选”应按照 [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/contributor-guides.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
