# 离线

Offline 传输层是一个占位实现，用于表明某个 realm 中没有可用的通信服务器。它通常用于测试和开发环境。

该传输层不会发出任何传入消息，并会忽略所有传出消息。如果你想要一个带有模拟行为的离线变体，请参见 [模拟器](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/simulator/) 代替。

URI 的形式如下：

```
offline:<自定义后缀>
```

在 `offline:` 前缀之后的内容未指定，且可由实现（或留空）按其认为合适的方式使用。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/offline.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
