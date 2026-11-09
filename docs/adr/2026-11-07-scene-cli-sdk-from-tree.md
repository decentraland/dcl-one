# ADR: The scene CLI vendors the SDK built from this tree

- Status: Accepted
- Date: 2026-11-07

## Context

`apps/dcl-scene-cli` ships the scene toolchain as `src/vendor/node_modules.zip`, extracted by
`init` so a scene builds and previews offline. Until now the archive was derived from a registry
install of `@dcl/sdk` on the `auth-server` line published by `decentraland/js-sdk-toolchain`
(`7.29.1-34986384248.commit-bb45080`), with two of this repository's behaviours applied as text
patches over the installed files: the peer-trust gate that lets a scene without an authoritative
server keep syncing, and a `tsconfig.ecs7.json` free of the options TypeScript 7 rejects. The
builder itself lived in another repository. A change to `library/protocol` or
`library/js-sdk-toolchain` (the SDK ADR of 2026-10-26) reached the CLI's scenes only through an
upstream publish and a pin bump, and the patched behaviour was not SDK source anyone could test.

## Decision

The archive is built from this tree, by `apps/dcl-scene-cli/scripts/build-base-blob.py`:

- Every `@dcl/*` package this tree builds is, in the archive, the output of this tree's build
  (`TREE_PACKAGES` in the builder): `@dcl/sdk`, `@dcl/ecs`, `@dcl/js-runtime`, `@dcl/react-ecs`,
  `@dcl/smart-items` (the smart-item runtime) and `@dcl/inspector-service` (the editor's
  data-layer host and composite readers, without the UI) from
  `make -C library/js-sdk-toolchain install build`, whose generated components and scene APIs
  come from `library/protocol`. Each is packed as npm would publish it (`pnpm pack`), stamped with
  the scaffold's `@dcl/sdk` pin (`src/templates/init/scene/package.json`), and every dependency
  between tree packages is rewritten to that version. The builder refuses an unbuilt toolchain.
- The only registry package the archive ships is `typescript`, the scaffold's pin. The
  registry manifest `src/vendor/registry/package.json` (with its `pnpm-lock.yaml`, installed
  with `pnpm install --frozen-lockfile`: hoisted, no scripts, no peer auto-install) names it as
  a dependency and, as devDependencies, what the chunk build and the declaration rollup resolve
  from the tree packages' own dependencies: `react`, `react-reconciler`, `@dcl/ecs-math`,
  `protobufjs`, `long`, `@protobufjs/utf8` and `mitt`; their code ends inside the chunks and
  their declarations inside the rollup. `@dcl/inspector-service`'s bundle carries its own
  dependencies (`@dcl/ecs`, `@dcl/rpc`, `ws`, ...), so no third-party runtime package ships.
  A pin a tree manifest owns (every exact dependency of the packed packages) and the scaffold's
  `typescript` are only checked against the registry manifest, never chosen there.
- The `auth-server` line of `decentraland/js-sdk-toolchain` is merged into
  `library/js-sdk-toolchain` (its delta over the 2026-10-23 snapshot: server-side scenes,
  `@dcl/sdk/server`, authoritative component writes, `CreatedBy`, the state-request retry, the
  `storage` and `scene-insights` commands). The two patched behaviours are now source there:
  `fromAuthority()` in `packages/@dcl/sdk/src/network/message-bus-sync.ts` trusts only
  `authoritative-server` unless `globalThis.__dclOneAuthoritative` is `false` (the bundle banner
  and the split loader set it from scene.json's `authoritativeMultiplayer` before any SDK code
  runs; a missing flag counts as `true`), and `types/tsconfig.ecs7.json` carries no
  `downlevelIteration` or `suppressExcessPropertyErrors` and resolves as `bundler`.
  `scripts/blob_overlays.py` checks that both reached the archive; nothing rewrites a shipped
  file.
- The data-layer host the editor connects to is the tree's `@dcl/inspector-service`, loaded from
  the scene's extracted archive unless `DCL_ONE_INSPECTOR_DIR` names an installed `@dcl/inspector`,
  whose own host then serves its UI. Composites that still carry `asset-packs::` component names
  are renamed on load by that package's `renameLegacyComposite` and, in Rust, by
  `dcl_scene_authoring::composite_norm`.
- The shipped version is `7.31.0-dcl-one.1`: the upstream base the tree's SDK derives from plus
  a dcl-one iteration, written once in the scaffold manifests and read from there by the builder
  and by `init.rs`. It is not on npm.
- The `Development` workflow's scene CLI job builds the SDK and the CLI under the Nix shell
  (`SDK_PROTOC=protoc`, so generated code comes from the shell's `protoc`), runs the crate's tests
  and the builder's `--check`, which rebuilds the archive in memory and fails on any entry that
  differs from the committed one. The archive is deterministic (sorted entries, fixed
  timestamps), so a change under `library/protocol`, `library/js-sdk-toolchain` or
  `src/vendor/registry` has to come with the regenerated archive and goldens.

## Consequences

- A protocol or SDK change and the scene CLI that ships it land in one pull request, and the CLI's
  scenes run the Explorer's protocol at that commit.
- The scaffold's `@dcl/sdk` pin cannot be reinstalled from the registry: `init` extracts the
  archive, and a scene that runs its own `pnpm install` keeps the extracted packages only while
  nothing asks the registry for that version. Publishing from this tree (SDK ADR follow-up 1)
  resolves it; until then the pin states where the packages come from.
- `library/js-sdk-toolchain` is the auth-server line, not upstream `main`: catching up with
  upstream (SDK ADR follow-up 6) means merging into that line.
- The editor's component set and the composite readers come from `@dcl/inspector-service` at
  the tree's commit; there is no snapshot of a registry inspector to refresh.
- Regenerating the archive needs the Nix shell (`node`, `pnpm`, `protoc`, `cargo`); the steps are
  in `apps/dcl-scene-cli/src/vendor/README.md`.
