# 签名登录

Signed Login 传输可供希望自定义将客户端分配到各个岛屿方式的服务器使用，从而在其架构中替代或封装 Archipelago。

Signed Login 不会直接连接到实时后端，而是发起一次 [签名获取](/docs/contributor/contributor-zh/shen-fen-yan-zheng/signed-fetch/) 以获取连接字符串。这个中间步骤使服务器能够采用他们想要的任何分配策略。

Signed Login URI 具有如下形式：

```
signed-login:https://comms.example.com/auth&param=value
```

在……之后的部分 `signed-login:` 前缀可以是任何有效的 URL。

### 用法

Signed Login 请求是使用以下内容构造的 [签名获取](/docs/contributor/contributor-zh/shen-fen-yan-zheng/signed-fetch/) 机制。

如果成功，响应的状态将为 `200` 并带有一个包含以下内容的 JSON 正文 *至少* 以下字段：

| 字段             | 类型       | 值                                     |
| -------------- | -------- | ------------------------------------- |
| `fixedAdapter` | `string` | 分配的传输 URI（例如 `livekit:` 或 `ws-room:`) |

例如：

```
{ 
  fixedAdapter: "ws-room:wss://comms.example.com/rooms/17"
}
```

如果失败，响应可以具有任何适当的状态码，并带有一个包含以下内容的 JSON 正文 *至少* 以下字段：

| 字段   | 类型       | 值                |
| ---- | -------- | ---------------- |
| `消息` | `string` | （可选）失败原因说明或错误代码。 |

JSON 正文中包含额外字段的响应也是有效的，而错误内容 `消息` 未作规定。实现可以利用这种灵活性来集成 `signed-login:` 到其应用中，只要它们认为合适即可。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/signed-login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
