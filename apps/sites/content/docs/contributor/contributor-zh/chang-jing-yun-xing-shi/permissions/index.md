# 权限

当场景需要访问敏感功能时，它们必须：

1. 声明相关的 [在其清单中声明权限](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions).
2. 使用 [RestrictedActions](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/restricted-actions/) 模块让运行时执行该操作（如果已授权）。

有关更详细的信息，请参阅这些页面。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/permissions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
