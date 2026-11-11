# 传输方式

一个 *传输层* 是一个客户端接口，可连接到通信 URI，并与对等方或服务交换实时消息。


> [!NOTE]
> 你可以通过开源的以下工具查看通信协议的实际运行情况并进行试验： [通信站](https://decentraland.github.io/comms-station/).


客户端可以自由地按自己的方式实现传输层（只要遵循其他客户端和服务所期望的协议），但 [推荐设计](#recommended) 如下所述，该设计已在实践中经过测试，并证明能够很好地与不同项目集成。\`

### 标准传输层 <a href="#types" id="types"></a>

该协议定义了 5 种传输类型：

* [WebSocket](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/websocket/) (`ws-room`）使用标准的 WebSocket 流。
* [LiveKit](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/livekit/) (`livekit`）使用基于 WebRTC 的开源 LiveKit 框架。
* [签名登录](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/signed-login/) (`signed-login`）发起经过签名的 HTTPS 请求，以从服务器获取动态分配的传输层。
* [离线](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/offline/) (`offline`）是一种虚拟传输层，表示当前环境中没有通信。
* [模拟器](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/simulator/) (`simulator`）是一种具有完全自定义行为的虚拟传输层，主要供开发人员调试其实现。

通过传输层接口发送和接收的消息始终相同（参见 [消息](/docs/contributor/contributor-zh/tong-xin/messages/)）。某些传输层可能会为了传输而将其封装在控制结构中，但在跨越 `传输层` 接口之前，应将其解包。

### 推荐设计 <a href="#recommended" id="recommended"></a>

这是一个最简 `传输层` 使用 TypeScript 编写的接口：

```ts
interface Transport {
  // 使用 URI 初始化 Transport：
  constructor(private uriWithConnectionParams: string)

  // 使用构造函数中提供的 URI 打开连接：
  connect(): Promise<void>

  // 关闭连接：
  disconnect(): Promise<void>

  // 向服务和所有对等方发送任意负载：
  send(packet: Packet): Promise<void>

  // 订阅来自服务和所有对等方的传入消息：
  on(event: 'receive', callback: (packet: Packet) => void): void
}
```

实际实现通常会扩展此接口，增加连接/断开连接和加入/离开事件、非广播发送、更严格的类型约束或特定语言的适配。

#### 传输层 URI

的值 `uriWithConnectionParams` 始终采用以下形式：

```
<类型>:<类型连接参数>
```

该 `<类型>` 对应于上面列出的某一种，而 `<类型连接参数>` 的格式取决于具体的传输层。它可以是 URL、不透明令牌，或任意数据。

该 [LiveKit](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/livekit/) 传输层例如使用一个 `wss` URL，并带有一个 `access_token` 参数来建立经过身份验证的连接：

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

相比之下， [WebSocket](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/websocket/) 传输层也使用一个 `wss` URL，但没有预先生成的令牌。它在连接后需要进行身份验证流程。

#### 创建传输层

由于每个 `传输层` 类都将支持某一种 URI，因此最好使用一个工厂方法：要么返回有效的 `传输层` ，要么立即失败。例如：

```ts
function createTransport(uriWithConnectionParams: string) {
  const type = getPrefix(urlWithConnectionParams)

  switch (type) {
    case 'ws-room': return new WsRoomTransport(uriWithConnectionParams)
    case 'livekit': return new LiveKitTransport(uriWithConnectionParams)
    // ... 其他受支持的实现...
  }

  throw new Error(`不支持的传输类型：${type}`)
}
```

客户端无需实现所有标准传输层。如果它们只打算使用已定义类型的一个子集，而不希望处理所有 URI，则可以自由拒绝不支持的类型。

功能更丰富的客户端，例如世界探索器，建议至少实现 [WebSocket](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/websocket/) 和 [LiveKit](/docs/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/livekit/) 传输层，这两种传输层目前被所有主要领域使用。

### 了解更多

前往 [特定传输类型](#types) 章节，阅读有关不同 [消息类型](/docs/contributor/contributor-zh/tong-xin/messages/) 的内容，或查看 \[\[通信演示站]]。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/transports.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
