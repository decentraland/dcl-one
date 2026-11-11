> 多人游戏模式和由服务器拥有的共享状态。

# 多人

多人游戏模式和由服务器拥有的共享状态。

| 示例                                                                                                                           | 它展示了什么                       | 维护者          | 标签                         | 来源                                                                                                               |
| ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------- | ------------ | -------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| [迷你游戏库](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/multiplayer-data-services/mini-games-library/)                    | 用于多人迷你游戏的玩家队列、记分板和支持逻辑。      | Decentraland | `多人游戏`, `记分板`, `游戏`        | [打开仓库](https://github.com/decentraland/sdk-mini-games)                                                           |
| [多人游戏服务器排行榜](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/multiplayer-data-services/authoritative-server-leaderboard/) | 由服务器拥有的排行榜，用于验证分数、存储总分并广播排名。 | Decentraland | `多人游戏`, `排行榜`, `服务器`, `存储` | [打开场景](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard) |

[浏览所有示例场景](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/multiplayer-data-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
