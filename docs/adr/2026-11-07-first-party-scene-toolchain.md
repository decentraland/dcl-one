# ADR: pnpm, first-party `@dcl/*` packages and smart items in the SDK

- Status: Accepted
- Date: 2026-11-07

## Context

The scene toolchain the CLI vendors (ADR 2026-11-07, the scene CLI vendors the SDK built from
this tree) still took three `@dcl/*` packages from the registry: `@dcl/ecs-math` (the SDK's
vector and matrix math), `@dcl/rpc` (the data-layer wire protocol the inspector stand-in speaks)
and `@dcl/asset-packs` (the smart-item runtime, published from the Creator Hub monorepo together
with an asset catalogue this repository does not ship). `library/js-sdk-toolchain` and
`library/protocol` installed with npm and `file:` links. The smart-item runtime reached a scene
bundle as compiled text copied out of `@dcl/sdk-commands` and `@dcl/inspector`'s nested copy of
`@dcl/asset-packs`, and composites named the smart-item components `asset-packs::*`, after a
package that is being deprecated.

## Decision

- **pnpm everywhere.** `library/js-sdk-toolchain` is a pnpm workspace: `pnpm-workspace.yaml`
  lists its packages, the snapshot scenes and the build fixtures, one `pnpm-lock.yaml` pins them
  and `nodeLinker: hoisted` keeps the flat layout its scripts expect. `library/protocol`,
  `library/ecs-math` and `library/rpc` are standalone pnpm projects with their own lockfiles.
  Workspace packages depend on each other with `workspace:*` and on the standalone projects with
  `link:`. `make install` is `pnpm install --frozen-lockfile`, with install scripts off and the
  tree's minimum release age for registry packages. npm is not used anywhere in the tree.
- **First-party `@dcl/ecs-math` and `@dcl/rpc`.** `library/ecs-math` (2.1.0, from
  decentraland/ecs-math) and `library/rpc` (1.1.2, from decentraland/rpc; its protobuf code is
  generated at build time with the tree's `protoc`) are library code with the protocol's
  Makefile pattern (`install`, `build`, `test`). `@dcl/sdk` links `@dcl/ecs-math`,
  `@dcl/sdk-commands` links `@dcl/rpc`, and the toolchain build compiles both before the SDK.
  The vendored blob packs them from the tree like the SDK packages, at the version their
  manifest carries.
- **Smart items are `@dcl/sdk/smart-items`.** The smart-item runtime (component definitions,
  actions, triggers, states, counters, tweens, admin tools and the admin toolkit UI, the
  versioning framework and the script runtime) lives in `packages/@dcl/sdk/src/smart-items`,
  built, typed and versioned with the SDK. A scene boots it with
  `initSmartItems(engine, { syncEntity }, players)` from `@dcl/sdk/smart-items`; the SDK build
  commands and the CLI's entrypoint do that for every editor scene. `~sdk/script-utils` stays a
  module generated per scene, carrying that scene's script instances and delegating to
  `@dcl/sdk/smart-items/scripts`. No part of the tree depends on `@dcl/asset-packs`; the asset
  catalogue it also carried stays a separate concern (the catalyst's `/v1/assetPacks` and the
  sites' seed catalogue).
- **The component namespace is `smart-items::`.** Every smart-item component is
  `smart-items::<Name>` (`smart-items::Actions`, `smart-items::Triggers`, `smart-items::Script`,
  …). A component number derives from its name, so the number changes with it. Composites
  written before the rename carry `asset-packs::` names; every reader of a composite (the SDK
  build commands, the CLI and its authoring library, the sites' composite parsers and the
  editor-scene hydration) renames that prefix on load, so an existing scene opens, builds and
  saves with the new names. CRDT files are regenerated from the composite, never renamed in
  place.
- **What still comes from the registry.** Tool and development dependencies of the SDK build
  commands that no scene ships: `@dcl/crypto`, `@dcl/crypto-middleware`, `@dcl/gltf-validator-ts`,
  `@dcl/hashing`, `@dcl/inspector`, `@dcl/linker-dapp`, `@dcl/mini-comms`, `@dcl/quests-client`,
  `@dcl/quests-manager` and `@dcl/schemas`, plus `@dcl/ts-proto`, `@dcl/quickjs-emscripten` and
  `@dcl/eslint-config` for the build itself. The vendored blob bundles no registry `@dcl/*`
  package; its registry extras are `typescript`, `protobufjs`, `react`, `react-reconciler`, `ws`,
  `mitt`, `long` and `@protobufjs/utf8`.

## Consequences

- One package manager, one lockfile per project; CI runs the protocol, ecs-math, rpc and SDK
  builds under the Nix shell, so generated code comes from the shell's `protoc`.
- A change to the smart-item runtime is an SDK change: it lands with the SDK, ships in the
  blob's smart chunk and is covered by the SDK's test suite.
- The registry `@dcl/inspector` UI, when a developer installs it for the visual editor, defines
  the smart-item components under their former names, so the data-layer host drops its edits to
  them and persists only core-component edits. Smart-item editing in that UI needs an inspector
  built against `@dcl/sdk/smart-items`.
- Catalogue composites served by the catalyst or the sites' seed catalogue still carry
  `asset-packs::` names and are renamed on load like any other legacy composite.
- The `@dcl/inspector` stand-in's component tables are regenerated with the rename applied by
  `apps/dcl-scene-cli/scripts/dump-inspector-tables.cjs`.
