# 架构

- [实体与组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/): 了解 Decentraland 场景中实体和组件的基础知识
- [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/): 了解系统如何用于更新场景状态
- [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/): 创建一个自定义组件来处理与实体相关的特定数据
- [组合文件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites/): 在运行时从组合文件生成一棵实体和组件树
- [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/): 了解如何获取具有共同组件的实体列表，以便更轻松地检查或更新它们。
- [订阅变更](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/subscribe-to-changes/): 检测组件中的变化，并在每次变化时运行函数
- [面向数据编程](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming/): 面向数据编程是一种强大的编程方法，能最大限度发挥性能。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
