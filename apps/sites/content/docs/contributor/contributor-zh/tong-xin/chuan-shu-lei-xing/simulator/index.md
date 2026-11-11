# 模拟器

Simulator 传输是一种完全任意的实现，除传输接口之外没有指定任何行为。

它可以根据需要发出传入消息并处理传出消息，而无需满足语义要求。实现可以选择永远不发出消息并忽略所有方法调用，但 [离线传输](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/offline/) 更适合这样做。

Simulator URI 看起来像这样：

```
simulator:<自定义后缀>
```

在 `simulator:` 前缀之后的内容未指定，且可由实现（或留空）按其认为合适的方式使用。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/simulator.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
