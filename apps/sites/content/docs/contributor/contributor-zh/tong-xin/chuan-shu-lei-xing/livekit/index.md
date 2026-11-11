# LiveKit

该 [LiveKit](https://livekit.io/) 该传输方式是受支持的传输中用途最广、效率最高的，也是最容易实现的。当客户端从其 realm 的 [Archipelago](/docs/contributor/contributor-zh/tong-xin/archipelago/) 服务接收到合适的连接字符串时，或者如果它被标明为固定传输时，应使用它。


> [!NOTE]
> 您可以通过使用开源的 [通信站](https://decentraland.github.io/comms-station/).


与自定义的 [WebSocket](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/websocket/) 传输方式不同，它使用第三方（开源）库，通过抽象出所有传输层逻辑和序列化，大大简化了开发过程。

LiveKit 传输 URI 以 `livekit:` 前缀开头，后接 `wss://` 服务器 URL。它们还包含一个预授权的 `access_token`，由 [Archipelago](/docs/contributor/contributor-zh/tong-xin/archipelago/) 在客户端进行身份验证时生成。

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

### 连接与身份验证

LiveKit 客户端库将所有连接和身份验证逻辑整洁地封装起来。

使用 LiveKit 传输时无需额外的身份验证步骤。由 `access_token` Archipelago 提供的内容已经过验证并获得授权。

### 交换消息

依赖 LiveKit 库的客户端可以轻松广播、发送直接消息并发布实时媒体流。

有效载荷是纯通信 [`数据包`](/docs/contributor/contributor-zh/tong-xin/messages/) 不包含任何额外封装。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/livekit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
