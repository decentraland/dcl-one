# livekit

Use the [LiveKit](https://livekit.io/) transport when the [Realm WebSocket connector](/docs/contributor/communications/archipelago/) supplies a LiveKit connection string or when the realm advertises LiveKit as its fixed transport.


> [!NOTE]
> You can see the LiveKit transport in action and play with its features using the open-source [Comms Station](https://decentraland.github.io/comms-station/).


Unlike the custom [websocket](/docs/contributor/communications/transport-types/websocket/) transport, it uses a third-party (open source) library, which greatly simplifies the development process by abstracting all transport-level logic and serialization.

LiveKit transport URIs begin with the `livekit:` prefix, followed by the `wss://` server URL. They also include a pre-authorized `access_token`, issued by Gatekeeper for a room the client may join.

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

### Connecting and Authenticating

The LiveKit client library neatly encapsulates all the connection and authentication logic.

There is no additional authentication step when using the LiveKit transport. The `access_token` carries authorization to join the room.

### Exchanging Messages

Clients relying on the LiveKit library can easily broadcast, send direct messages and publish live media feeds.

Payloads are plain comms [`Packets`](/docs/contributor/communications/messages/) without any additional wrapping.
