# Record: Pulse ENet wire interop with an ENet-CSharp client

- Status: Evidence record for [Share the pinned ENet dependency between Pulse and the native Explorer](2026-10-28-shared-pulse-enet.md)
- Date: 2026-11-23
- Issue: [#1550](https://github.com/decentraland/dcl-one/issues/1550)

The ADR keeps native interoperability as a separate evidence gate. This record holds the
live evidence for that gate: a Unity Explorer client, whose native ENet transport is
ENet-CSharp, connected to dcl-one's Pulse and exchanged player state with clients that
use the vendored `rusty_enet` and with the Bevy web client. These are observations from
deployed runs, not a CI result.

## Setup

- Deployment: dcl.dev, running dcl-one's servers. Realm `dcl-one-poc`; Pulse at
  `dcl-one-poc-rt.decentraland.zone`, ENet over UDP port 7777.
- Unity client: the build of
  [decentraland/unity-explorer#10686](https://github.com/decentraland/unity-explorer/pull/10686)
  (`v0.199.0-alpha-feat/realm-scene-pointers-pulse-server-7a360e3`), which adds the
  `--pulse-server` option. Launched with `--pulse true --pulse-server
  dcl-one-poc-rt.decentraland.zone --realm https://dcl.dev --gatekeeper-url
  https://dcl.dev/comms-gatekeeper`.
- Other clients: 20 `comms-e2e` protocol bots (`rusty_enet`, native ENet) and the Bevy web
  client (WebTransport).

## Observations

### 2026-11-20, 18:48 to 18:54 UTC (dcl.dev at `395565680c`, then `60c7ac7aa6`)

1. The Unity client's own Pulse diagnostics read Transport Connected and Authenticated
   True. The handshake and the three-channel framing therefore work between ENet-CSharp
   and the patched `rusty_enet` header layout.
2. While the 20 bots walked on the same parcel, the Unity client received 2.7 to 3.0 KB/s
   at about 175 packets/s. It sent its own state at about 18 to 23 B/s at rest.
3. Every one of the 20 bots received a Pulse `PlayerJoined` for the Unity client 591 to
   645 ms after entering and held it at its position (1606.0, 1606.0). The server relayed
   the ENet-CSharp client's state to `rusty_enet` clients. The `comms-e2e` scenario
   `crowd-visit` passed: all expected participants were caught.
4. The Bevy web client on the same Pulse held the Unity client as a remote player
   (`/transport_state` showed 21 Pulse players with the bots; `/foreign_movement` listed
   it) and drew its avatar. The Unity client received the Bevy client's movement over
   Pulse at 203 B/s, 8 packets/s, while it walked.

### 2026-11-23, 09:31 to 09:45 UTC

5. The same Unity build authenticated to dcl.dev's Pulse again and received the join of a
   signed-in Bevy client, with its profile version, over Pulse.

## Related finding

Before [#1808](https://github.com/decentraland/dcl-one/pull/1808) (merged 2026-11-20,
60 s future-skew tolerance), a client whose clock ran 0.3 s fast was refused at the
handshake. That was a clock rule, not a wire-format problem.

## What this does not establish

- The ignored native test `native_csharp_all_lanes_and_controlled_sender_throttle` in
  `library/vendor/rusty_enet/src/test.rs` needs a separately built ENet-CSharp probe
  (`PULSE_NATIVE_PROBE`, `PULSE_NATIVE_TRACE`, `--include-ignored`). It is not run by the
  Development workflow, which runs `cargo test --lib` on the vendor workspace without
  `--include-ignored` (`tooling/dev-test.sh`), and the probe source is not in this
  repository. A default green run does not prove native interop.
- The runs cover one Unity build, one deployment and a short window. They do not cover
  lane-by-lane behaviour under sender throttle, long sessions or load.
