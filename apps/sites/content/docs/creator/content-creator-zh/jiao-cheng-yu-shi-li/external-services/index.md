> 将场景连接到外部 API、数据源和第三方服务器。

# 外部服务

将场景连接到外部 API、数据源和第三方服务器。

| 示例                                                                                                                   | 它展示了什么                             | 维护者          | 标签                          | 来源                                                                                               |
| -------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ------------ | --------------------------- | ------------------------------------------------------------------------------------------------ |
| [来自电子表格的数据](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services/spreadsheet-data/)                  | 由 Google Sheet 驱动的场景内容，无需重新部署即可更新。 | Decentraland | `API`, `电子表格`, `视频`, `穿戴设备` | [打开仓库](https://github.com/decentraland/Read_Spreadsheet_Scene)                                   |
| [第三方留言簿](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services/third-party-guest-book/)               | 由外部 HTTP 服务支持的留言簿。                 | Decentraland | `API`, `存储`                 | [打开场景](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)               |
| [第三方排行榜](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services/third-party-leaderboard/)              | 由外部 HTTP 服务支持的持久化排行榜。              | Decentraland | `API`, `存储`, `排行榜`          | [打开场景](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)                 |
| [第三方玩家身份验证](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services/third-party-player-authentication/) | 为发送到外部服务的请求验证玩家身份。                 | Decentraland | `API`, `身份验证`, `玩家`         | [打开场景](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

[浏览所有示例场景](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/external-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
