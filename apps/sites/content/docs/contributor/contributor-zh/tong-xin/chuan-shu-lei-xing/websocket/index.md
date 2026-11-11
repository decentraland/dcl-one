# WebSocket

WebSocket 传输是 Decentraland 通信协议支持的岛屿消息传输方式之一。客户端在从其所在 realm 的 [Archipelago](/docs/contributor/contributor-zh/tong-xin/archipelago/) 服务，或者如果它被标记为固定传输时，应使用它。

在 WebSocket 传输的上下文中，岛屿被称为一个 *房间*。这两个名称指的是同一件事：一群彼此靠近、会交换更新和聊天消息的玩家。

连接字符串以 `ws-room:` 前缀开头，后面跟着一个 `wss://` URI，用于指定的房间。它们看起来像这样：

```
ws-room:wss://comms.example.com/rooms/<room-id>
```

如果 websocket URI 没有明确指定协议， `wss://` 则默认采用。

### 连接

该传输在 HTTPS 上使用标准的 WebSocket 协议。客户端可以使用任何现成的标准实现来建立连接。

### WebSocket 数据包

来自 WebSocket 传输的所有消息都使用 [`WsPacket`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L78) 结构进行序列化。它们实现了一组协议级的连接与认证消息，以及一个用于客户端消息的容器类型。


> [!NOTE]
> 该 [`WsPacket`](#websocket-packets) 结构不应与消息 \[`数据包`]\[Packet] 混淆。它是 WebSocket 传输特有的额外包装层。实际的通信消息包含在 [`WsPeerUpdate`](#wspeerupdate) 类型中。


| 字段   | 类型   | 值                                                                                                                                                                                                                                             |
| ---- | ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `消息` | `枚举` | <p>以下之一 <code>WsIdentification</code>, <code>WsChallengeRequired</code>, <code>WsSignedChallenge</code>,<br><code>WsWelcome</code>, <code>WsPeerJoin</code>, <code>WsPeerLeave</code>, <code>WsKicked</code> 或 <code>WsPeerUpdate</code>.</p> |

### 认证

在开始向他人转发消息之前，客户端必须通过签名一个挑战字符串来完成身份验证。即使传输 URI 是在 [Archipelago](/docs/contributor/contributor-zh/tong-xin/archipelago/) 经过上一轮认证之后获得的，也仍然需要这样做。

客户端加入岛屿时发送的第一条消息是 [`WsIdentification`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64)，其中包含一个公开地址。它将收到 [`WsChallengeRequired`](#wschallengerequired)，并且客户端发送一个 [`WsSignedChallenge`](#wssignedchallenge) 作为回复。

```
  .----------.                .--------.
  |  服务器  |                | 客户端 |
  '----+-----'                '---+----'
       ⋮                          |
       ⋮                  连接 |
       o - - - - - - - - - - - - -|
       |                          |
       |                          |
       |         WsIdentification |
       |<-------------------------+
       +------------------------->|
       | WsChallengeRequired      |
       |                          |
       |                          |
       |        WsSignedChallenge |
       |<-------------------------+
       +------------------------->|
       | WsWelcome                |
       |                          |

```

如果流程成功完成，客户端将收到一个 [`WsWelcome`](#wswelcome) ，并可以开始向对等方发送消息。

***

### `WsIdentification` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64) <a href="#wsidentification" id="wsidentification"></a>

| 字段   | 类型    | 值                  |
| ---- | ----- | ------------------ |
| `地址` | `字符串` | 客户端的公开 Ethereum 地址 |

***

### `WsChallengeRequired` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L48) <a href="#wschallengerequired" id="wschallengerequired"></a>

| 字段                  | 类型    | 值                        |
| ------------------- | ----- | ------------------------ |
| `challenge_to_sign` | `字符串` | 由服务器提供、需要签名作为身份证明的字符串    |
| `already_connected` | `字符串` | 服务器给客户端的提示，表示之前的连接可能已关闭。 |

***

### `WsSignedChallenge` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L56) <a href="#wssignedchallenge" id="wssignedchallenge"></a>

| 字段                | 类型    | 值                                                                                |
| ----------------- | ----- | -------------------------------------------------------------------------------- |
| `auth_chain_json` | `字符串` | 序列化的 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 以挑战签名结尾。 |

***

### `WsWelcome` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L11) <a href="#wswelcome" id="wswelcome"></a>

| 字段                | 类型                    | 值                |
| ----------------- | --------------------- | ---------------- |
| `别名`              | `uint32`              | 服务器为客户端会话生成的 ID  |
| `peer_identities` | `map<uint32, string>` | 当前所有对等方的地址，按别名索引 |

### 连接性

定义了三条消息，帮助客户端跟踪其对等方以及自身状态。

[`WsPeerJoin`](#wspeerjoin) 当另一位客户端连接到房间并成功完成身份验证时会收到，而 [`WsPeerLeave`](#wspeerleave) 则在对等方断开连接时收到。

此外还有 [`WsKicked`](#wskicked) 消息，它会通知客户端其连接即将被关闭以及关闭原因。在当前实践中，服务器能够将客户端踢出房间的主要原因是：当服务器策略禁止同时连接到另一个房间时，他们同时连接到了另一个房间。

***

### `WsPeerJoin` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L19) <a href="#wspeerjoin" id="wspeerjoin"></a>

| 字段   | 类型       | 值                                         |
| ---- | -------- | ----------------------------------------- |
| `别名` | `uint32` | 在……中发送的服务器生成 ID [`WsWelcome`](#wswelcome) |
| `地址` | `字符串`    | 新对等方的 Ethereum 地址                         |

***

### `WsPeerLeave` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L27) <a href="#wspeerleave" id="wspeerleave"></a>

| 字段   | 类型       | 值                  |
| ---- | -------- | ------------------ |
| `别名` | `uint32` | 已断开连接的对等方的服务器生成 ID |

***

### `WsKicked` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L74) <a href="#wskicked" id="wskicked"></a>

| 字段   | 类型    | 值             |
| ---- | ----- | ------------- |
| `原因` | `字符串` | 服务器对关闭连接原因的说明 |

### 客户端消息

要发送通讯消息 [消息](/docs/contributor/contributor-zh/tong-xin/messages/)，客户端会将其包装在 [`WsPeerUpdate`](#wspeerupdate) 结构中。这将传输控制消息类型与对等方之间实际发送的消息区分开来。

```
.-------------------.
| WsPacket          |
|  .--------------. |
|  | WsPeerUpdate | |
|  | .----------. | |
|  | |  Packet  | | |
|  | '----------' | |
|  '--------------' |
'-------------------'

```

***

### `WsPeerUpdate` [**↗ 源码**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L34) <a href="#wspeerupdate" id="wspeerupdate"></a>

| 字段           | 类型       | 值                                                                 |
| ------------ | -------- | ----------------------------------------------------------------- |
| `from_alias` | `uint32` | 发送者的服务器生成 ID                                                      |
| `body`       | `字节`     | 序列化后的 [消息](/docs/contributor/contributor-zh/tong-xin/messages/) 被包装的 |
| `unreliable` | `bool`   | 发送者在传递此消息时优先考虑速度还是可靠性                                             |

客户端发送时必须将 `from_alias` 字段设为 `0` ，服务器会在将其传递给对等方之前填入正确的标识符。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/chuan-shu-lei-xing/websocket.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
