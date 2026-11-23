# Record: Pulse ENet wire interop with an ENet-CSharp client

- Status: Evidence record for [Share the pinned ENet dependency between Pulse and the native Explorer](2026-10-28-shared-pulse-enet.md)
- Date: 2026-11-23
- Issue: [#1550](https://github.com/decentraland/dcl-one/issues/1550)

The ADR keeps native interoperability as a separate evidence gate. This record holds the
live evidence for that gate: a Unity Explorer client, whose native ENet transport is
ENet-CSharp, connected to dcl-one's Pulse and exchanged player state with clients that
use the vendored `rusty_enet` and with the Bevy web client. The record also holds one-off
runs of the vendored crate's native interop test against ENet-CSharp. These are
observations, not a standing CI check.

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
   645 ms into the run (all 20 were admitted by 417 ms) and held it at its position
   (1606.0, 1606.0). The server relayed
   the ENet-CSharp client's state to `rusty_enet` clients. The `comms-e2e` scenario
   `crowd-visit` passed: all expected participants were caught.
4. The Bevy web client on the same Pulse held the Unity client as a remote player
   (`/transport_state` showed 21 Pulse players with the bots; `/foreign_movement` listed
   it) and drew its avatar. The Unity client received the Bevy client's movement over
   Pulse at 203 B/s, 8 packets/s, while it walked.

### 2026-11-23, 09:31 to 09:45 UTC

5. The same Unity build authenticated to dcl.dev's Pulse again and received the join of a
   signed-in Bevy client, with its profile version, over Pulse.

## Native interop test, 2026-11-23

`native_csharp_all_lanes_and_controlled_sender_throttle` in
`library/vendor/rusty_enet/src/test.rs` runs the vendored `rusty_enet` host against an
ENet-CSharp client over loopback. Its .NET probe is not in this repository. For this
record it was rebuilt from the test's contract, on ENet-CSharp 2.4.9 as Unity Explorer
ships it:

- Wrapper and lanes: `ENet.cs` and `ENetChannel.cs` of the `com.decentraland.pulse.transport`
  package, at decentraland/Pulse `994b07ee16`, the commit unity-explorer's
  `Packages/packages-lock.json` pins.
- Native library: unity-explorer's `Explorer/Assets/Plugins/ENet/runtimes` at `4c85b8603e`
  (main, 2026-11-19). The Linux run used `libenet.so`, SHA-256 `8269fcb7…c537`; the Windows
  run used `enet.dll`, `0a03cd54…dba3`.

The probe connects as Unity Explorer's `ENetTransport.ConnectAsync` does: one peer, three
channels. It sends one packet per lane with the shipped flags (`Reliable`, `Unthrottled`,
`Unsequenced`), then checks what the host sends back.

| Run | Native library | Result |
| --- | --- | --- |
| Local, Windows x64 | `enet.dll` | `1 passed; 0 failed; 0 ignored` |
| [GitHub Actions](https://github.com/decentraland/dcl-one/actions/runs/45157354026/job/137633594123), Ubuntu | `libenet.so` | `1 passed; 0 failed; 0 ignored` |
| Local negative control: the host sends lane 1 without the throttle bypass | `enet.dll` | lane 1 received none of the eight bypass packets; the probe and the test failed |

In both passing runs:

- The host received the probe's 4096 bytes on lane 0 (reliable), 32 bytes on lane 1
  (sequenced unreliable) and 32 bytes on lane 2 (unsequenced).
- With the host's send throttle at 0, its ordinary unreliable `[99]` on lane 0 never
  arrived. Its eight packets `[100]` to `[107]` on lane 1, sent with the per-packet bypass,
  all arrived in order.
- After the host restored its throttle, each lane received a full-size and a one-byte packet,
  in order on lanes 0 and 1, and the disconnect carried reason 16.

## Related finding

Before [#1808](https://github.com/decentraland/dcl-one/pull/1808) (merged 2026-11-20,
60 s future-skew tolerance), a client whose clock ran 0.3 s fast was refused at the
handshake. That was a clock rule, not a wire-format problem.

## What this does not establish

- The native test stays ignored by default, and no CI job runs it. The Development
  workflow runs `cargo test --lib` on the vendor workspace without `--include-ignored`
  (`tooling/dev-test.sh`), and the probe is not committed. A later change to the vendored
  crate is checked against ENet-CSharp only if someone rebuilds the probe and runs the
  test. A default green run does not prove native interop.
- The live runs cover one Unity build, one deployment and a short window. The native
  test covers one ENet-CSharp version over loopback. Neither covers long sessions or load.
