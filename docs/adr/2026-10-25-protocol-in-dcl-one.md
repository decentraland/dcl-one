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
- One change is not additive: `PBAvatarEquippedData` field 3 is `wearables`, as
  `decentraland/protocol`'s `main` defined it after this snapshot and published in
  `@dcl/protocol@next`, and `force_render`, which held 3 here and in `experimental-bevy`, is field
  4, a renumbering the wire-compatibility check was exempted for, in that file only, until it
  merged. With the new numbers, every definition in `@dcl/protocol@next` exists
  here under the same number, name and type. The `experimental-bevy`
  protocol and SDK took the same numbering (decentraland/protocol#510,
  decentraland/js-sdk-toolchain#1675), and the bridge scene pins that SDK, which moves it 31
  commits forward on that branch (AvatarNametag, `getPlayerLanguage`, input and pointer fixes,
  `sdk-commands` bundle and deploy changes), not only to the new field number. The inspector's
  Bevy agent (`apps/creator-hub/packages/inspector/agents/bevy`) still pins an SDK that reads
  `force_render` at 3; it only checks the player for null. The engine sets no
  `wearables[].visible` yet. Field 4 is not
  reserved on `decentraland/protocol`'s `main`, so the same collision can recur until it is.
- `tooling/proto-source-check.py` (run by `structure.yml`) fails on a `decentraland.*`
  proto anywhere else. Every Rust consumer (`dcl_component`, `comms`, `social`,
  `dcl-scene-authoring`, `dcl-scene-cli`) compiles from `library/protocol/proto` at build
  time; no generated code is committed.
- Changes to the Explorer's protos trigger the Explorer workflows (`library/protocol/proto`
  is one of their inputs, in `tooling/bevy-ci.py` and the `paths:` filters), and the Nix
  Play build's source set includes it.
- `.github/workflows/protocol-release.yml` publishes `@dcl/protocol` under the `next` dist-tag on
  a push to `main` that changes `library/protocol`, as `1.0.0-<run>.commit-<sha>` (oddish stamps
  it; the manifest keeps `1.0.0`), through npm trusted publishing and the
  `npm-release-protocol` environment, which only `main` may deploy to
  ([releases ADR](2026-11-09-releases-per-scope.md)). A pull
  request from this repository publishes what its run packed as `@dcl-regenesislabs/protocol`
  under the shared `pr` dist-tag from `.github/workflows/protocol.yml`, and the PR's CI status
  comment names the exact version; fork pull requests publish nothing. The package has no
  versioned release and no branch dist-tag from here.

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

1. `decentraland/protocol` to stop publishing `next`: while it publishes, the tag points at
   whichever repository pushed last, and a commit it lands on `main` is missing here until it is
   ported. `next` also needs a trusted publisher on `@dcl/protocol` naming
   `protocol-release.yml` and the `npm-release-protocol` environment, and the previews one on
   `@dcl-regenesislabs/protocol` naming `protocol.yml`.
2. Import `js-sdk-toolchain` beside it (`library/js-sdk-toolchain`), reading
   `library/protocol` directly; then its publishing and the `auth-server` branch.
3. Decide the protocol repository's `experimental` and `protocol-squad` branches, which no
   consumer tracks today.
