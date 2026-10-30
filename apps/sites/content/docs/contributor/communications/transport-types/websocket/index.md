# websocket

The websocket transport is one of the island messaging transports supported by the Decentraland comms protocol. Clients should use it when they receive an appropriate connection string from their realm's [Archipelago](/docs/contributor/communications/archipelago/) service, or if it's indiciated as a fixed transport.

In the context of the websocket transport, an island is referred to as a _room_. Either name indicates the same thing: a group of nearby players that exchange updates and chat.

Connection strings begin with the `ws-room:` prefix, followed by a `wss://` URI for the specific room. They look like this:

```
ws-room:wss://comms.example.com/rooms/<room-id>
```

In the abscence of an explicit protocol for the websocket URI, `wss://` is assumed.

### Connecting

The transport uses the regular websocket protocol over HTTPS. Clients can open a connection using any standard implementation at their disposal.

### Websocket Packets

All messages from the websocket transport are serialized using the [`WsPacket`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L78) structure. They implement a set of protocol-level connectivity and authentication messages, plus a container type for client messages.


> [!NOTE]
> The [`WsPacket`](/docs/contributor/communications/transport-types/websocket/#WsPacket) structure should not be confused with the message \[`Packet`]\[Packet]. It's an additional wrapping layer specific to the websocket transport. Actual comms messages are contained in the [`WsPeerUpdate`](/docs/contributor/communications/transport-types/websocket/#WsPeerUpdate) type.


| Field     | Type   | Value                                                                                                                                                                                                                                            |
| --------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `message` | `enum` | <p>One of <code>WsIdentification</code>, <code>WsChallengeRequired</code>, <code>WsSignedChallenge</code>,<br><code>WsWelcome</code>, <code>WsPeerJoin</code>, <code>WsPeerLeave</code>, <code>WsKicked</code> or <code>WsPeerUpdate</code>.</p> |

### Authentication

Before they can start relaying messages to others, clients must authenticate by signing a challenge string. This is required even when the transport URI was obtained from [Archipelago](/docs/contributor/communications/archipelago/) after a previous round of authentication.

The first message a client sends when joining an island is [`WsIdentification`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64), which contains a public address. It will be responded with [`WsChallengeRequired`](/docs/contributor/communications/transport-types/websocket/#WsChallengeRequired), and clients send a [`WsSignedChallenge`](/docs/contributor/communications/transport-types/websocket/#WsSignedChallenge) in reply.

```
  .----------.                .--------.
  |  Server  |                | Client |
  '----+-----'                '---+----'
       ⋮                          |
       ⋮                  Connect |
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

If the flow is completed successfully, the client will receive a [`WsWelcome`](/docs/contributor/communications/transport-types/websocket/#WsWelcome) and can start sending messages to peers.

***

**`WsIdentification`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64)

| Field     | Type     | Value                                     |
| --------- | -------- | ----------------------------------------- |
| `address` | `string` | The public Ethereum address of the client |

***

**`WsChallengeRequired`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L48)

| Field               | Type     | Value                                                                      |
| ------------------- | -------- | -------------------------------------------------------------------------- |
| `challenge_to_sign` | `string` | The server-provided string to be signed as proof of identity               |
| `already_connected` | `string` | A server hint to clients, indicating that prior connections may be closed. |

***

**`WsSignedChallenge`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L56)

| Field             | Type     | Value                                                                                              |
| ----------------- | -------- | -------------------------------------------------------------------------------------------------- |
| `auth_chain_json` | `string` | A serialized [authentication chain](/docs/contributor/authentication/authchain/) ending with the challenge signature. |

***

**`WsWelcome`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L11)

| Field             | Type                  | Value                                                |
| ----------------- | --------------------- | ---------------------------------------------------- |
| `alias`           | `uint32`              | A server-generated ID for the client's session       |
| `peer_identities` | `map<uint32, string>` | The addresses of all current peers, indexed by alias |

### Connectivity

Three messages are defined to help clients keep track of their peers and their own status.

[`WsPeerJoin`](/docs/contributor/communications/transport-types/websocket/#WsPeerJoin) is received when a fellow client connects to the room and successfully authenticates, while [`WsPeerLeave`](/docs/contributor/communications/transport-types/websocket/#WsPeerLeave) is received when peers disconnect.

There's also the [`WsKicked`](/docs/contributor/communications/transport-types/websocket/#WsKicked) message, which informs clients that their connection is about to be closed and the reason for it. In current practice, the main reason a server can kick a client from a room is because they've simultaneously connected to another room, when that is forbidden by server policy.

***

**`WsPeerJoin`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L19)

| Field     | Type     | Value                                                                 |
| --------- | -------- | --------------------------------------------------------------------- |
| `alias`   | `uint32` | The server-generated ID sent in [`WsWelcome`](/docs/contributor/communications/transport-types/websocket/#WsWelcome) |
| `address` | `string` | The new peer's Ethereum address                                       |

***

**`WsPeerLeave`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L27)

| Field   | Type     | Value                                            |
| ------- | -------- | ------------------------------------------------ |
| `alias` | `uint32` | The server-generated ID of the disconnected peer |

***

**`WsKicked`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L74)

| Field    | Type     | Value                                                         |
| -------- | -------- | ------------------------------------------------------------- |
| `reason` | `string` | The server's explanation of why the connection will be closed |

### Client Messages

To send comms [messages](/docs/contributor/communications/messages/), clients wrap them in the [`WsPeerUpdate`](/docs/contributor/communications/transport-types/websocket/#WsPeerUpdate) structure. This differentiates the transport control message types from the actual messages sent between peers.

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

**`WsPeerUpdate`** [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L34)

| Field        | Type     | Value                                                                            |
| ------------ | -------- | -------------------------------------------------------------------------------- |
| `from_alias` | `uint32` | The sender's server-generated ID                                                 |
| `body`       | `bytes`  | The serialized [message](/docs/contributor/communications/messages/) being wrapped                          |
| `unreliable` | `bool`   | Whether the sender prioritized speed or reliability for deliverying this message |

Clients must set the `from_alias` field to `0` when sending, and the server will fill it with the correct identifier before delivering it to peers.
