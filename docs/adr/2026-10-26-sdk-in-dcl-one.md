# ADR: The creator SDK lives in `library/js-sdk-toolchain`

- Status: Accepted
- Date: 2026-10-26

## Context

The creator SDK (`@dcl/sdk`, `@dcl/ecs`, `@dcl/react-ecs`, `@dcl/js-runtime`,
`@dcl/sdk-commands`, `@dcl/playground-assets`) lived in `decentraland/js-sdk-toolchain` and
reached this repository only as npm versions pinned by Sites, the template games, the editor scene
and the scene CLI templates. It generates its components and scene APIs from `@dcl/protocol`,
pinned by hand to a `next` snapshot: a protocol change reached creators weeks later, through a
bump PR, and the SDK could not express what the Explorer here implements (the components
`library/protocol` carries since the protocol ADR). With `library/protocol` the one proto source,
the SDK belongs beside it.

## Decision

`library/js-sdk-toolchain` is a first-party snapshot of `decentraland/js-sdk-toolchain` `main`
(`afd7f79`, 2026-10-23), keeping its Apache-2.0 `LICENSE`, its npm layout (six packages under
`packages/@dcl`, per-package lockfiles, `file:` links between them) and its build (`make install
build test prepare`). Its `.github`, agent files and editor configuration were left behind.

- `@dcl/protocol` is `file:../protocol` (root) and `file:../../../../protocol` (`sdk-commands`):
  npm links `library/protocol`, so the generated components, scene APIs and `out-ts` types come
  from the one proto source. `make install` builds the protocol package first; `update-protocol`
  is gone, since there is nothing to bump.
- No published package depends on `@dcl/protocol`. `@dcl/sdk-commands`, the one that used it at
  runtime, generates the two messages it needs (`local_development`, realm `about`) into
  `src/generated` during `make build`, with the options `@dcl/protocol`'s own `out-ts` uses, so the
  SDK ships the protocol at its commit and never waits on a protocol release.
  `scripts/check-no-local-packages.cjs`, run by `sdk.yml`, still fails a tarball that names a
  `file:` link.
- `AvatarShape` and `AvatarEquippedData` keep `forceRender` optional for scenes: the protocol's
  `repeated` field would otherwise make the generated type require it, so the two extended
  components from the SDK's `experimental-bevy` branch serialize an omitted value as `[]`, and a
  composite that names them registers the extended definitions. No export is removed, but the
  generated types follow `library/protocol`, so some narrow: `PBAvatarShape.forceRender` and
  `PBAvatarEquippedData.forceRender` are required, `PBEngineInfo.totalRuntimeF64` and five arrays
  on `PBGltfContainerLoadingState` are new required fields, and `PBMaterial`, `PBMeshCollider` and
  `PBMeshRenderer` gain a `$case: 'gltf'` variant (an exhaustive `switch` stops compiling). The
  first release from here lists these.
- `protoc` comes from `SDK_PROTOC` when set: a name on `PATH` (`SDK_PROTOC=protoc` in the Nix
  shell, or brew's on a Mac without Rosetta) or a path, resolved to an absolute path. It is not
  `PROTOC`, which prost reads for the Rust crates. CI leaves it unset and uses the SDK's download
  of 3.20.1, so generated code in a PR comes from that version.
- `.github/workflows/sdk.yml` runs lint, build, test, the CLI smoke test and `prepare` on changes
  under `library/js-sdk-toolchain` or `library/protocol`. A push to `main` also publishes `ecs`,
  `js-runtime`, `react-ecs`, `sdk-commands` and `sdk` (`playground-assets` is built and tested, not
  published) under the `next` dist-tag as `<latest patch+1>-<run>.commit-<sha>` (oddish derives
  the version from the registry's `latest`; the manifests keep `7.0.0` as upstream does, and
  `npm publish` ships the tarballs through trusted publishing). A pull request from this
  repository gets the tarballs its run built published under the shared `pr` dist-tag, one tag for
  every PR, by `.github/workflows/sdk-preview.yml`, and a sticky comment names the exact version to
  install; fork pull requests publish nothing.
  Versioned releases are cut by hand ([releases ADR](2026-11-09-releases-per-scope.md)). The
  manifests name this repository, which trusted publishing requires.
- The packages publish as `@dcl-regenesislabs/*` until the `@dcl` names can be published from here;
  switching back is the six `name` fields. The code and the dependency keys keep the `@dcl` names,
  the build installs siblings under those keys (`@dcl/ecs@file:../ecs`), and `make prepare` pins a
  sibling as an npm alias (`npm:@dcl-regenesislabs/ecs@<version>`) when its name differs from the
  key. A scene installs `@dcl/sdk@npm:@dcl-regenesislabs/sdk@<version>` and keeps importing
  `@dcl/sdk`. oddish reads the dist-tags of the manifest's name, so `next` snapshots under this
  scope start from the placeholder (`7.0.0-<run>.commit-<sha>`) rather than from `@dcl/sdk@latest`.
- Snapshots (`test/snapshots/**`, `playground-assets.api.md`) are regenerated locally
  (`make update-snapshots`) whenever `library/protocol` changes what `@dcl/ecs` generates; the
  snapshot test fails on a stale one (with `UPDATE_SNAPSHOTS` it also rewrites it) and the
  workflow uploads the regenerated files as the `sdk-generated` artifact.

## Consequences

- One pull request can change a proto, the engine and the SDK together; the SDK release that
  follows ships the protocol at that commit.
- `@dcl/sdk@next` describes the Explorer dcl-one ships once publishing moves here; the bridge and
  movement scenes stop needing the `experimental-bevy` dist-tag.
- `library/` holds a second npm project that is not a `dcl-*` crate (as `catalyst-api/` and
  `protocol/` already are); its lockfiles and `node_modules` stay its own.

## Follow-ups

1. Releases and `next` are published from here, through trusted publishers on `sdk.yml` and
   `sdk-release.yml` (a new package needs one manual publish before it accepts one); still to do:
   the `@dcl` names, stop publishing from `decentraland/js-sdk-toolchain`, the
   rest of the PR tooling (the e2e realm, the playground). The first release notes carry the
   narrowed types above.
2. The `auth-server` branch as a dcl-one branch published through a manual `publish-tag` run.
3. Fold the rest of the SDK's `experimental-bevy` layer (react-ecs scroll and render-to-texture,
   drag events, `setGltfMesh`, …) into `main` here.
4. `make lint-packages` (inherited) checks only the root manifest: its `packages/*/package.json`
   glob does not match `packages/@dcl/*`.
5. Sites, the template games, the editor scene and the scene CLI templates consume one SDK version.
6. Catch up with what `decentraland/js-sdk-toolchain` `main` merged after the snapshot: the
   skybox, sun and fog components (its #1638) need `library/protocol` to carry the protocol's
   #495 first; `TouchScreenControls.showDefault()` (its #1649) does not.
