# API 参考

- [测试与反馈](/docs/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/alpha-testing-guidelines/): 关于如何尝试 SDK7 并提供反馈的指南
- [发行说明](/docs/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/releases/): 过去一年中 SDK 和 Creator Hub 的变化，已筛选为影响创作者的改动。
- [API 参考](/docs/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/v7-alpha/): Decentraland SDK7 的完整 API 参考，由 js-sdk 工具链生成。
- [版本支持协议](/docs/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/version-support-agreement/): 版本支持协议


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/api-can-kao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
