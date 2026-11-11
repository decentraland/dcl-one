# WebSocket

社交服务是一个管理 Decentraland 内好友关系的服务器。它利用 Matrix 平台，作为处理好友关系更新的权威来源，例如请求、接受、拒绝、取消和删除。

### 使用场景

社交服务解决以下与好友关系相关的使用场景：

* 请求添加好友
* 接受好友请求
* 拒绝好友请求
* 取消好友请求
* 移除好友
* 列出我的所有好友
* 列出我与另一位用户之间的共同好友

### 背景

在引入社交服务之前，好友数据是直接从 Matrix 中存储和检索的。这导致了好友流程中的各种问题以及不可靠的信息。此外，实现新功能也很困难。由于 Matrix 主要处理聊天功能，社交服务作为一个代理，在与 Matrix 交互之前应用必要的业务逻辑。

### 架构

#### 版本历史

1. [第一个版本](https://adr.decentraland.org/adr/ADR-113)：最初，社交服务是一个 REST API，只提供用于列出所有好友和共同连接的 GET 端点。它缺少与更新或发送请求相关的功能。
2. [第二个版本](https://adr.decentraland.org/adr/ADR-189)：此版本引入了好友流程的更新。为应对这些变化，实现了一个 WebSocket RPC 服务器。该服务器处理核心消息，维护与客户端的开放连接，以提供及时更新。WebSocket 连接还支持诸如 [在线状态跟踪](https://github.com/decentraland/adr/blob/420e63926d20166ae832e3de62dd9e3f2370cd49/content/ADR-246-poc-presence.md) 等社交服务内的功能。

#### 网址

**.zone 域名**

用于测试环境以及使用测试网时的开发环境服务器。

* [使用 Matrix 登录](https://social.decentraland.zone)
* [REST API](https://social.decentraland.zone)
* [WebSocket API](https://rpc-social.decentraland.zone)

**.org 域名**

用于生产环境服务器以及使用主网时。

* [使用 Matrix 登录](https://social.decentraland.org)
* [REST API](https://social.decentraland.org)
* [WebSocket API](https://rpc-social.decentraland.org)

### 使用 Javascript 客户端

要使用 Javascript 客户端，请参阅 <https://github.com/decentraland/social-rpc-client-js#using-the-client>

### 参考客户端实现

* [Matrix 客户端](https://github.com/decentraland/matrix-client)
* [JS 客户端](https://github.com/decentraland/social-rpc-client-js)
* [Rust 参考客户端](https://github.com/decentraland/social-client)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/she-jiao-fu-wu/ws.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
