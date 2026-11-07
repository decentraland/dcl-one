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

- Every `@dcl/*` package in the archive is the output of this tree's builds (`TREE_PACKAGES` in
  the builder): `@dcl/sdk`, `@dcl/ecs`, `@dcl/js-runtime` and `@dcl/react-ecs` from
  `make -C library/js-sdk-toolchain install build`, whose generated components and scene APIs
  come from `library/protocol`, and `@dcl/ecs-math` and `@dcl/rpc` from `library/ecs-math` and
  `library/rpc` (ADR 2026-11-07, pnpm, first-party `@dcl/*` packages and smart items in the
  SDK). Each is packed as npm would publish it (`pnpm pack`); the SDK packages are stamped with
  the scaffold's `@dcl/sdk` pin (`src/templates/init/scene/package.json`), the other two keep
  their manifest's version, and every dependency between tree packages is rewritten to the
  shipped version. The builder refuses an unbuilt toolchain.
- Everything else the archive ships or bundles comes from the registry through
  `src/vendor/registry/{package.json,pnpm-lock.yaml}`, installed with
  `pnpm install --frozen-lockfile` (hoisted, no scripts, no peer auto-install). Two of those pins
  are owned elsewhere and only checked: `typescript` against the scaffold, `protobufjs` against
  `library/protocol`. No `@dcl/*` package comes from the registry.
- The `auth-server` line of `decentraland/js-sdk-toolchain` is merged into
  `library/js-sdk-toolchain` (its delta over the 2026-10-23 snapshot: server-side scenes,
  `@dcl/sdk/server`, authoritative component writes, `CreatedBy`, the state-request retry, the
  `storage` and `scene-insights` commands). The two patched behaviours are now source there:
  `fromAuthority()` in `packages/@dcl/sdk/src/network/message-bus-sync.ts` trusts only
  `authoritative-server` when the loader sets `globalThis.__dclOneAuthoritative` and peers
  otherwise, and `types/tsconfig.ecs7.json` carries no `downlevelIteration` or
  `suppressExcessPropertyErrors` and resolves as `bundler`. `scripts/blob_overlays.py` keeps the
  two remaining rewrites (the minimal `protobufjs`, the `@dcl/inspector` stand-in) and checks that
  both tree-sourced behaviours reached the archive.
- The shipped version is `7.31.0-dcl-one.1`: the upstream base the tree's SDK derives from plus
  a dcl-one iteration, written once in the scaffold manifests and read from there by the builder
  and by `init.rs`. It is not on npm.
- The `Scene CLI journeys` workflow gains a `blob` job that builds the SDK and the CLI under the
  Nix shell (`SDK_PROTOC=protoc`, so generated code comes from the shell's `protoc`) and runs the
  builder's `--check`, which rebuilds the archive in memory and fails on any entry that differs
  from the committed one. The archive is deterministic (sorted entries, fixed timestamps), so a
  change under `library/protocol`, `library/js-sdk-toolchain` or `src/vendor/registry` has to come
  with the regenerated archive and goldens.

## Consequences

- A protocol or SDK change and the scene CLI that ships it land in one pull request, and the CLI's
  scenes run the Explorer's protocol at that commit.
- The scaffold's `@dcl/sdk` pin cannot be reinstalled from the registry: `init` extracts the
  archive, and a scene that runs its own `pnpm install` keeps the extracted packages only while
  nothing asks the registry for that version. Publishing from this tree (SDK ADR follow-up 1)
  resolves it; until then the pin states where the packages come from.
- `library/js-sdk-toolchain` is the auth-server line, not upstream `main`: catching up with
  upstream (SDK ADR follow-up 6) means merging into that line.
- The `@dcl/inspector` stand-in's component tables are a snapshot of a registry inspector
  (7.43.2) with the smart-item components renamed to the `smart-items::` namespace;
  `scripts/dump-inspector-tables.cjs` refreshes them from an installed inspector.
- Regenerating the archive needs the Nix shell (`node`, `pnpm`, `protoc`, `cargo`); the steps are
  in `apps/dcl-scene-cli/src/vendor/README.md`.
