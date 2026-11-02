# ADR: The protocol lives in `library/protocol`

- Status: Accepted
- Date: 2026-10-25

## Context

The `.proto` contract between scenes, the SDK, the explorers and the services lived in
`decentraland/protocol` and reached this repository as copies: the Explorer kept 125 files
under `crates/dcl_component/src/proto`, and `dcl-scene-authoring`, `dcl-scene-cli`, `comms`
and `social` each kept their own subset. None recorded the revision it came from, and they
drifted in both directions: the Explorer's copy carried ten components and several fields
that `protocol/main` never received (they sat in its `experimental-bevy` branch, or only in
the Explorer), while `main` had grown five components, RPCs and fields the Explorer lacked.
The `/play` HUD depends on the Explorer-only components, so building the Explorer against
`protocol/main` does not compile, and `@dcl/sdk@latest` cannot express what the Explorer
dcl-one ships implements.

## Decision

`library/protocol` is the protocol: a first-party snapshot of `decentraland/protocol`
(`main` at `70f2654`, `experimental-bevy` at `329ba0e` merged in, and the Explorer's
additions reconciled on top; the same change is proposed to that repository as
`feat/bevy-protocol-reconciliation`). It keeps its Apache-2.0 `LICENSE`, its buf
configuration and its npm package layout, and its `proto/` tree is the **only** `.proto`
source in this repository.

- The Explorer compiles from it: `crates/dcl_component/build.rs` resolves
  `library/protocol/proto` through `DCL_ONE_ROOT` (`DCL_PROTOCOL_PROTO` overrides it for a
  build outside the repository) and its copy is deleted entirely (the v1 friendships proto
  it alone still carried went with #123).
- The protocol's wire-compatibility check (`buf breaking`) compares against this
  repository's `main`, so every change to `proto/` is additive or explicitly reserved.
- `tooling/proto-source-check.py` (run by `structure.yml`) fails on a `decentraland.*`
  proto anywhere else, the Explorer included. The copies that remain are allowlisted by
  path until the change that points their consumer at `library/protocol` removes them;
  nothing is added to that list.
- Changes to the Explorer's protos trigger the Explorer workflows (`library/protocol/proto`
  is one of their inputs, in `tooling/bevy-ci.py` and the `paths:` filters), and the Nix
  Play build's source set includes it.
- `@dcl/protocol` is still published by `decentraland/protocol`; the package here is marked
  `private` until publishing moves. Publishing (`next` on `main` pushes, a dist-tag from a
  branch on a manual run, with npm trusted publishing) is a separate change, as is the SDK.

What was reconciled, all additive (`buf breaking` clean, no `ecs_component_id` or
field-number collision): the components `gltf_node` 1200, `gltf_node_state` 1201,
`ui_scroll_result` 1202, `ui_canvas` 1203, `global_light` 1206, `texture_camera` 1207,
`camera_layers` 1208, `avatar_movement_info` 1500, `avatar_movement` 1501, `camera_layer`
1503; rfc4 comms `SceneDrivenAnimation`, `PositionPrecision`, the `Packet`, `Movement` and
`MovementCompressed` fields that carry them (tags 10000/10001) and `ChatReaction.emoji`
(10001) / `remove` (10002); `AvatarModifierArea.movement_settings` with
`AvatarMovementSettings` and `AvatarControlType`; `CameraModeArea.cinematic_settings` with
`CinematicSettings`; `EngineInfo.total_runtime_f64`; `UiText.outline_width` and
`outline_color`. Not taken: the Explorer's `optional` on `ChatReaction.emoji_index` (a
synthetic oneof the wire check rejects; the engine reads the implicit-presence field) and the
places where its copy was behind `main`.

## Consequences

- The Explorer, the HUD and later the SDK describe one protocol, the one dcl-one ships.
  A proto change and the engine change that implements it land in one pull request.
- Consumers of `@dcl/protocol@next` outside this repository (Unity, Godot) gain messages
  their explorer does not implement once the reconciliation reaches `protocol/main`, as the
  Explorer already lives with the five `main`-only components. Additive only.
- Deleting the Explorer's copy changes `rendering/bevy-explorer/source`; its workflows build
  and publish the engine from `library/protocol` as they do for any engine change.
- `rendering/bevy-explorer/source` is no longer self-contained: it needs the repository root
  (`DCL_ONE_ROOT`) or `DCL_PROTOCOL_PROTO`.

## Follow-ups

1. Publish `@dcl/protocol` from here and stop publishing from `decentraland/protocol`.
2. Import `js-sdk-toolchain` beside it (`library/js-sdk-toolchain`), reading
   `library/protocol` directly; then its publishing and the `auth-server` branch.
3. Point `dcl-scene-authoring`, `dcl-scene-cli` and `social` at `library/protocol` (`comms`
   already does; `recipients` enters `library/protocol` first) and shrink the allowlist to nothing.
4. Decide the protocol repository's `experimental` and `protocol-squad` branches, which no
   consumer tracks today.
