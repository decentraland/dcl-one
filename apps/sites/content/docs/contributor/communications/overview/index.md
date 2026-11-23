# overview

The Decentraland communications system, or simply _comms_, is the real-time messaging protocol that handles interaction between players in a realm.

Some of these interactions are initiated by players, others are automatically handled by clients under the hood. Some examples:

* Text and voice chat
* Position updates as players move around
* Avatar updates when players change their appearance


> [!NOTE]
> You can see the comms protocol in action and experiment with it using the open-source [Comms Station](https://decentraland.github.io/comms-station/).


Pulse synchronizes avatar movement and groups nearby players into temporary clusters called islands. Gatekeeper controls room access and issues LiveKit credentials. The signed WebSocket connector delivers those assignments to clients.

<span id="lifecycle"></span>

## Client Life-cycle

1. Read the realm’s `/about` response to discover its communications adapter.
2. If the realm advertises the signed [Realm WebSocket connector](/docs/contributor/communications/archipelago/), connect and authenticate. Explorer also connects to Pulse for avatar synchronization.
3. Use the transport URI in the assignment to join the island’s media room.
4. Process later assignments as the selected room changes. Close the connections when the session ends.

## Connections

The realm connector uses WebSocket. Pulse uses ENet or WebTransport. LiveKit transports media over WebRTC.

Go to the [Transports](/docs/contributor/communications/transports/) or [Realm WebSocket connector](/docs/contributor/communications/archipelago/) sections to learn more, or a specific transport page for details about it.


## Authentication

Connections to comms are authenticated by having clients sign a server-provided challenge, using the scheme described in the [authentication chain](/docs/contributor/authentication/authchain/) section.

Head over to the [Realm WebSocket connector](/docs/contributor/communications/archipelago/) page or see a specific transport to learn more about authentication flows.


## Islands

An island is a temporary proximity group. Pulse updates membership as players move. Gatekeeper maps assignments to rooms and the connector supplies the URI needed to join. See the [Realm WebSocket connector](/docs/contributor/communications/archipelago/) for the assignment flow.

## Messages

Messages in the comms protocol are binary blobs serialized using [protocol buffers](https://github.com/protocolbuffers/protobuf), wrapped in a [Packet](/docs/contributor/communications/messages/#packet) structure.


> [!NOTE]
> When using the word _message_ in the context of comms, we'll always be referring to the binary message protocol, not to chat messages exchanged between players.


There are several different message types, for a variety of real-time interaction flows. Head over to the [messages](/docs/contributor/communications/messages/) section to learn more.
