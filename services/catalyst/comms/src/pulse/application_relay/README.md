# Scoped application relay

Dormant implementation for opaque application payloads inside the one LiveKit room a signed
admission grant authorizes. Media stays in LiveKit.

## Runtime status

No environment option activates the relay. The public handshake capability mask excludes
application relay, so legacy admission cannot grant a scope. Internal tests exercise the
retained relay implementation and the gatekeeper's room-authority claim/proof boundary.

## Wire contract

- Internal admission requires a live authority; payload delivery requires an established room scope.
- A join carries the JWT `header.payload` and an HMAC proof binding the v4 challenge's 32-byte value, wallet, signing session and the signing-input hash; the result's nonce field stays empty. Identity, room and guest status come only from the gatekeeper's answer; Pulse never parses claims or holds the LiveKit secret.
- The authority checks room purpose, bans, world access and island ownership. Permission lasts at most 2 s from request start; renewal is never a fresh join. Denial or expiry removes the scope and sends a reliable self-left notice.
- Listeners stay read-only for avatar state; a listener with its own `canPublishData` grant may publish application messages.
- Limits: 1024 rooms, 64 participants per room, 16 scopes per connection, 128 concurrent authority calls, 16 joins/s, 120 messages and 128 KiB per second per sender. Payloads: reliable 3072 B, unreliable 1024 B, unsigned credentials 3072 B (inside the 4096-byte reliable frame).
- A full recipient queue retires that recipient's scope; a reliable send over the sender limits retires the sender's scope; an over-limit unreliable send is dropped. Membership notices may use an 8 KiB per-connection reserve (2 MiB per host). Any other reliable enqueue failure retires the connection.
- Each room has a roster version (1 at first join, +1 per join and leave), carried on the join result and membership notices. A send stamped with a version skips later joiners; a stamped reliable send that predates a leave (log of 64 leaves, 10 s) gets one send-failed notice naming those identities and echoing the payload. Version 0 keeps the unstamped behaviour. Unreliable sends are never answered.
- A connection holding a scope gets a 5 s ENet timeout; a WebTransport connection that sends input and is silent 5 s has its scopes retired on the next tick (its session is kept).
