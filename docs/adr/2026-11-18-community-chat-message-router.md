# ADR: The community chat message router is its own process

- Status: Accepted
- Date: 2026-11-18

## Context

The Explorer sends each community chat message and reaction on the LiveKit private messages
room to one server participant, the message router (`message-router-dev-0`, or
`message-router-prd-0` on decentraland.org). The router checks that the sender is a member of
the community and relays the packet to the other members with the author stamped in; the
Explorer trusts the stamped author only from that identity. Decentraland runs the router as
`comms-message-sfu` (TypeScript, pinned at d38ccd0885ad23028604e9a9fcc788cdfcbb81fe). A
dcl-one node minted the private messages room token but had no router, so community chat did
not work against it.

A server participant in a LiveKit room links libwebrtc. Only the gatekeeper holds the LiveKit
secret.

## Decision

- **Own crate and binary.** `services/catalyst/message-router` (package
  `catalyst-message-router`, layer 5 in `docs/module-layers.toml`) is a member of the catalyst
  workspace and builds the binary `catalyst-message-router`. The `catalyst` binary does not
  link it, so a libwebrtc fault stops the router and not `serve`. Its LiveKit participant is
  behind the crate feature `livekit`; without it, the routing, membership, token and metrics
  code builds and tests without libwebrtc.
- **The Explorer's LiveKit SDK.** The workspace takes `livekit` from the commit of this
  repository that the Explorer's patched client-sdk-rust names
  (`rendering/bevy-explorer/patches/client-sdk-rust/source.toml` `rev`); a test of the crate
  checks that both name the same commit.
- **No LiveKit secret in the router.** The gatekeeper mounts
  `POST /internal/message-router/token/v1` when `MESSAGE_ROUTER_KEY` is set. The router signs
  each request with the same key (HMAC-SHA256 over method, path, timestamp, nonce and body
  hash; the contract is `shared::message_router`). The body names only the replica; the
  gatekeeper's `MESSAGE_ROUTER_IDENTITY_PREFIX` fixes the identity, and the grant joins the
  private messages room to subscribe and publish data only. The router asks again on every
  connection. The gateway keeps `/internal/message-router` private.
- **Shared SQL reads.** The router reads `social.community_members` and
  `social.communities` (`active`, `suspended`) on its own small pool and writes nothing (`DATABASE.md`
  "Message router workload"). No schema change.

## Rejected options

- **The router inside `serve`:** a libwebrtc crash would take down every API of the node.
- **The router with the LiveKit secret, as upstream:** a second holder of the key that signs
  every room's tokens.
- **A separate workspace for the router:** a second lock file and toolchain setup for one
  binary that shares the node's database and contract crates.

## Consequences

- The catalyst `Cargo.lock` names a git source of this repository, as the Explorer's does, and
  gains the LiveKit SDK's dependency tree (about 64 packages). Cargo locks an optional
  dependency too, so every fresh resolve and every Nix vendor of the workspace fetches that
  commit even though `catalyst` never compiles it. The commit holds only the patched source's
  published paths while its root manifest lists upstream's members, so
  `infra/packages/catalyst.nix` vendors its crates without loading that workspace
  (`vendoredCrates` in `infra/packages/patched-source.nix`, shared with `bevy-explorer.nix` and `cast-bot.nix`).
- A deployment runs the binary beside `serve`, with the same `MESSAGE_ROUTER_KEY` on both and
  `MESSAGE_ROUTER_IDENTITY_PREFIX` on `serve`. The APIs image carries it at
  `/bin/catalyst-message-router` (`infra/packages/message-router.nix`), run as a container of
  its own with that entrypoint. The process exits non-zero when it cannot stay in the room, so
  its supervisor must restart it on exit.
- Behaviour, configuration and metrics are in
  [comms ownership](../../services/catalyst/docs/comms/comms-crate.md#message-router).
