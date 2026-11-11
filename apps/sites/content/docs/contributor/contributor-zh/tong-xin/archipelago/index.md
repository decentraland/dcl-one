# Archipelago

Archipelago 是领地服务，它会将附近玩家分组到不同岛屿中，在他们移动时重新分配，并提供连接到实际后端所需的信息，后端将转发他们的消息。


> [!NOTE]
> 你可以通过开源的 [通讯站](https://decentraland.github.io/comms-station/).


来查看 Archipelago 协议的实际运行并进行实验。 [客户端生命周期](/docs/contributor/contributor-zh/tong-xin/overview/#lifecycle)).

与 Archipelago 服务交换的所有消息都使用 protocol buffers 编码，定义见 Decentraland [协议仓库](https://github.com/decentraland/protocol).

### 连接 <a href="#connecting" id="connecting"></a>

首先，客户端必须打开一个安全的 websocket 连接（`wss:`）到该领地的 `/archipelago/ws` 端点。

一旦连接成功，客户端就有一段由领地策略定义的时间窗口（默认 60 秒）来发送认证流程中的每条消息。 [认证](#authenticating) 流程。


> [!WARNING]
> 虽然大型领地运行 Archipelago 服务，但较小的领地可能会选择为所有玩家提供一个固定的后端连接字符串。如果存在，该 URI 可在领地的 `comms.fixedAdapter` 属性中找到，位于领地的 `/about`.


在这些情况下，将无法使用动态岛屿分配，也不应为了该目的使用领地的 RPC 接口。

### 正在认证 <a href="#authenticating" id="authenticating"></a>

在打开到 Archipelago 的连接后，客户端必须先请求并对服务发送的挑战进行签名，以验证其身份。

客户端发送的第一条消息是一个 [`ChallengeRequestMessage`](#challengerequestmessage) ，其中包含他们的以太坊地址（即其公钥）。他们将收到一个 [`ChallengeResponseMessage`](#challengeresponsemessage) ，其中包含一串随机生成的待签名字符串，并且必须回复一个 [`SignedChallengeMessage`](#signedchallengemessage).

该 [`SignedChallengeMessage`](#signedchallengemessage) 携带一个 JSON 序列化的 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) ，它以所提供的地址开头，以挑战签名结尾。

如果服务成功验证签名，则客户端已通过认证，并将收到一个 [`WelcomeMessage`](#welcomemessage).

```mermaid
时序图
    参与者 客户端
    参与者 Archipelago
    
    注：位于 Client 与 Archipelago 之间：连接
    客户端->>Archipelago：ChallengeRequestMessage
    Archipelago->>客户端：ChallengeResponseMessage
    客户端->>Archipelago：SignedChallengeMessage
    Archipelago->>客户端：WelcomeMessage
```

### 发送心跳 <a href="#heartbeat" id="heartbeat"></a>

在会话期间，客户端必须定期发送一个 [`Heartbeat`](#heartbeat-message) 消息，以便让 Archipelago 保持最新所需信息，从而发放岛屿分配。


> [!NOTE]
> 对通讯客户端，建议的心跳频率大约是每秒一次更新。


如果客户端停止发送 [`Heartbeat`](#heartbeat-message) 消息，Archipelago 可能（取决于当前策略）关闭连接。

### 获取岛屿分配 <a href="#assignment" id="assignment"></a>

在第一次心跳之后不久，Archipelago 会向客户端发送其第一条 [`IslandChangedMessage`](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L17).

主要字段是 `conn_str`，它可用于初始化传输并连接到该岛。其值通常如下所示：

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

第一个 `:` 前面的标签是传输类型，其余部分是该类型的专用 URI。它可以包含预授权令牌或其他参数。

在会话期间，Archipelago 可能随时发送新的分配，原因有多种：

1. 位置变化：客户端报告自己已远离岛上的其他人。
2. 岛屿请求：客户端请求被分配到特定岛屿。
3. Archipelago 策略：服务决定创建或拆分岛屿，以更好地平衡人口。

客户端必须监听这些分配，按指示关闭和打开传输连接，并在需要时更改所使用的传输类型。

### 客户端消息

#### `ChallengeRequestMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L54) <a href="#challengerequestmessage" id="challengerequestmessage"></a>

由客户端在会话开始时作为第一条消息发送，用于启动认证流程。

| 字段        | 类型       | 值      |
| --------- | -------- | ------ |
| `address` | `string` | 用户的地址。 |

该 `address` 该字段必须派生自 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 所呈现内容的第一个私钥。

***

#### `SignedChallengeMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L58) <a href="#signedchallengemessage" id="signedchallengemessage"></a>

在收到一个之后由客户端发送， [`ChallengeResponseMessage`](#challengeresponsemessage)以完成认证流程。

| 字段                | 类型       | 值                                                                                        |
| ----------------- | -------- | ---------------------------------------------------------------------------------------- |
| `auth_chain_json` | `string` | 一个 JSON 序列化的 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 以挑战签名结尾。 |

在 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) 中的第一个键必须对应于原始 [`ChallengeRequestMessage`](#challengerequestmessage).

***

#### `Heartbeat` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L62) <a href="#heartbeat-message" id="heartbeat-message"></a>

客户端每隔固定时间（通常每秒一次）发送，用于向 Archipelago 更新其位置和/或请求岛屿分配。

| 字段             | 类型        | 值               |
| -------------- | --------- | --------------- |
| `位置`           | `位置`      | 客户端在世界地图中的三维位置  |
| `desired_room` | `string?` | 客户端希望被分配到的岛屿 ID |

第一个 `Heartbeat` 消息之后，会紧接着收到来自 Archipelago 的一个 [`IslandChangedMessage`](#islandchangedmessage) 。不过，后续更新与岛屿分配无关。客户端不应期望对 `Heartbeat` 作出响应。

当 `desired_room` 参数被包含时，服务会尝试满足该请求，但不保证会重新分配到该岛。这取决于 Archipelago 的策略（例如岛屿人数限制）。

### 服务器消息

#### `ChallengeResponseMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L8) <a href="#challengeresponsemessage" id="challengeresponsemessage"></a>

由 Archipelago 作为对 [`ChallengeRequestMessage`](#challengerequestmessage)

| 字段                  | 类型       | 值                                                                            |
| ------------------- | -------- | ---------------------------------------------------------------------------- |
| `challenge_to_sign` | `string` | 用于签名并创建一个 [认证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/) |
| `already_connected` | `bool`   | 该用户密钥是否已有现有连接                                                                |

***

#### `WelcomeMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L13) <a href="#welcomemessage" id="welcomemessage"></a>

在成功认证后由 Archipelago 发送。

| 字段        | 类型       | 值                    |
| --------- | -------- | -------------------- |
| `peer_id` | `string` | 已认证客户端的唯一标识符（通常是其地址） |

***

#### `IslandChangedMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L17) <a href="#islandchangedmessage" id="islandchangedmessage"></a>

当客户端被（重新）分配到一个岛屿时，由 Archipelago 发送。

描述。

| 字段               | 类型                      | 值                                                                          |
| ---------------- | ----------------------- | -------------------------------------------------------------------------- |
| `island_id`      | `string`                | 新岛屿的 ID                                                                    |
| `from_island_id` | `string?`               | 旧岛屿的 ID，如果这是一次重新分配                                                         |
| `conn_str`       | `string`                | 该岛屿的连接字符串 [transport](/docs/contributor/contributor-zh/tong-xin/transports/). |
| `peers`          | `map<string, Position>` | 描述。                                                                        |

接收到一个的客户端 `IslandChangedMessage` 应结束与该岛后端的连接，并连接到 `conn_str`.

该 `peers` 中给出的那个。该字段包含岛中所有对等节点当前的身份和位置，以便客户端填充其初始集合。在此之后，它们应依赖通过岛屿 [transport](/docs/contributor/contributor-zh/tong-xin/transports/) 收到的消息来获取位置更新。

***

#### `KickedMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#LL37C1-L38C1) <a href="#kickedmessage" id="kickedmessage"></a>

由 Archipelago 在关闭连接前发送。

| 字段   | 类型             | 值                   |
| ---- | -------------- | ------------------- |
| `原因` | `KickedReason` | Archipelago 关闭连接的原因 |

该 `原因` 字段的标准值有：

* `KR_NEW_SESSION`：另一个连接使用相同的密钥完成了认证。

***

#### `JoinIslandMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L29) <a href="#joinislandmessage" id="joinislandmessage"></a>

当某个对等节点被分配到客户端所在岛屿时，由 Archipelago 发送。

| 字段          | 类型       | 值                  |
| ----------- | -------- | ------------------ |
| `island_id` | `string` | 岛屿的标识符             |
| `peer_id`   | `string` | 对等节点的唯一标识符（通常是其地址） |

该 `island_id` 字段将与客户端当前分配一致。

***

#### `LeftIslandMessage` [↗ 源码](https://github.com/decentraland/protocol/blob/9a568b16b2eafb134177329ba670c1451be8a169/proto/decentraland/kernel/comms/v3/archipelago.proto#L24) <a href="#leftislandmessage" id="leftislandmessage"></a>

当某个对等节点从客户端所在岛屿移除时，由 Archipelago 发送。

| 字段          | 类型       | 值                  |
| ----------- | -------- | ------------------ |
| `island_id` | `string` | 岛屿的标识符             |
| `peer_id`   | `string` | 对等节点的唯一标识符（通常是其地址） |

该 `island_id` 字段将与客户端当前分配一致。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/tong-xin/archipelago.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
