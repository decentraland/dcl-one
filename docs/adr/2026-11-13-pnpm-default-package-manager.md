# ADR: pnpm is the default package manager

- Status: Accepted
- Date: 2026-11-13

## Context

dcl-one has no JavaScript workspace at its root: each JavaScript project keeps its own manifest
and lockfile. `apps/sites`, `apps/ui3`, `apps/ui3/editor-scene`, `library/ui`,
`tooling/shop-parity` and `tooling/shop-trade-journey` installed with pnpm, and
`library/js-sdk-toolchain`, `library/protocol` and the Explorer's bridge scene moved to it with
the [local hashing library](2026-11-13-hashing-in-dcl-one.md). The Explorer's end-to-end suite,
its `react-web` and `apps/creator-hub` installed with npm.

pnpm refuses dependency install scripts unless a project allows them, holds back versions
published too recently, and lays out `node_modules` so that an import of an undeclared package
fails. One package manager also means one set of install rules to review.

## Decision

Internal development and CI install with pnpm. This replaces the package-manager part of the
[SDK](2026-10-26-sdk-in-dcl-one.md) and [Creator Hub](2026-11-09-creator-hub-in-dcl-one.md)
decisions; the rest of both stands.

- A project commits `pnpm-lock.yaml` and pins pnpm 12.21.0 in its root `packageManager`. pnpm 12
  is a native program; through corepack it downloads and verifies that program on first use. Each
  lockfile opens with the pinned pnpm's own packages and their integrity
  (`packageManagerDependencies`), and a frozen install fails until it does.
  CI and the build scripts install with `--frozen-lockfile` on that version, through corepack
  (`corepack enable`, or `corepack pnpm@12.21.0`, the `PNPM` default of the SDK and protocol
  Makefiles) or through the development shell's pnpm, which switches to the pinned version. The
  Nix builds run the Nix package set's pnpm and ignore the pin.
- `library/ui`'s `package.json` is an input of its generated components, so changing its pin is a
  component regeneration (`pnpm figma port prepare`, then `port verify`).
- pnpm does not read `package-lock.json`. A project moves with `pnpm import`, which writes
  `pnpm-lock.yaml` from it, and then deletes `package-lock.json`. The import resolves again
  where npm had settled a dist-tag or a peer on its own, so the two version sets are compared
  before the old lockfile goes.
- pnpm settings live in `pnpm-workspace.yaml` (`ignoreScripts`, `saveExact`, `allowBuilds`,
  `overrides`, `nodeLinker`, `minimumReleaseAge`). pnpm 11 and 12 read only registry, auth and
  network keys from `.npmrc`, and no manifest's `overrides`.
- No dependency runs an install script unless `allowBuilds` names it with the reason.
- Tools a project depends on run from its lockfile (`pnpm exec`), never through `npx`, which
  fetches a release when it runs. A tool in no lockfile runs through `pnpm dlx` (the Creator Hub's
  `skills@1.7.0` and `@dcl/cdn-uploader@next`), which keeps pnpm's release wait unless the project
  exempts the package.
- Script arguments follow the script name with no separator (`pnpm run e2e --inworld`). pnpm
  hands a `--` to the script as an argument, where npm removes it.
- Published `@dcl/*` packages carry no pnpm-specific field or configuration, so their users
  install with npm, yarn or pnpm. They are versioned, packed and published with pnpm: the
  Creator Hub's through `.github/actions/creator-hub-npm-publish`, the Explorer's through
  oddish's `package-manager` input. pnpm records no `gitHead`, so the Creator Hub action sets it.
- `tooling/package-manager-check.py` fails on an npm or yarn lockfile, on a project or workflow
  off the pinned pnpm (`PNPM_VERSION`), on a pin carrying another release's integrity
  (`PNPM_SHA512`), on an `.npmrc` key pnpm does not read, and on a workflow or a project script
  that runs any npm command, packing, publishing and registry queries included, yarn, or
  anything through `npx`.
- A script that starts another script of its own project uses `pnpm run` where its project's
  `make` already calls `pnpm` by name (`apps/creator-hub`). The SDK starts pnpm through corepack,
  which leaves it off its scripts' `PATH`, so its scripts run the other script's command directly.

Where the layout differs from pnpm's default:

- `library/js-sdk-toolchain`, `library/protocol`, the scenes built with the SDK
  (`apps/ui3/editor-scene`, the Explorer's bridge and movement scenes, the Creator Hub's Bevy
  agent) and
  `apps/creator-hub` set `nodeLinker: hoisted`. The SDK's published type declarations reach a
  sibling package by relative path, and the Creator Hub's packaging and tests read one flat
  `node_modules`. In these projects an import of an undeclared package is not caught at install.
- `library/js-sdk-toolchain` is one pnpm workspace with a single lockfile, `library/hashing`
  included, and keeps its workspace packages out of the root `node_modules`
  (`hoistWorkspacePackages: false`), where 11.28 started placing them. It drops the published SDK
  `@dcl/quests-client` asks for, which `sdk-commands` never loads and pnpm 12 places in the root.
- `apps/creator-hub` holds, through overrides, the versions its packaged app shipped under npm
  where pnpm would deduplicate them to another locked version, each until its dependent's next
  upgrade, and keeps `@dcl/quests-client`'s
  request for the `latest` `@dcl/sdk` on the SDK it pins. Its `electron-builder.cjs` makes
  electron-builder collect dependencies by walking `node_modules`, since the pnpm collector picks
  registry copies of workspace packages.

What stays on npm:

- What the products run for a creator, and the test scenes that stand in for a creator's project:
  the SDK's `npm install` in a scene, `library/js-sdk-toolchain/test/snapshots`,
  `test/build-ecs/fixtures`, `library/dcl-scene-build/testdata` and `init-test-scene`, and the
  npm the Creator Hub bundles.
- Instructions written for creators or for users of a published package: the packages' READMEs,
  the creator docs in `apps/sites` and `apps/dcl-scene-cli`, and the steps of the Creator Hub's
  `local-package-chain` runbook that act on a scene.
- `react-web`'s `bundle:native` starts the bridge scene's export with `npm --prefix`, since pnpm
  started through corepack is not on the `PATH` of the scripts it runs.

## Consequences

- The Creator Hub's published packages keep their `dependencies` and packed file lists;
  `@dcl/inspector`'s bundle carries one React instead of two.
- `@dcl/asset-packs` and `@dcl/inspector` import packages their published `dependencies` do not
  declare, which the flat install hides. Declaring them changes what their users install and is a
  separate decision.
- The electron-builder setting depends on that tool's internals and is checked on each upgrade.
- Another pnpm version is one change: each project's pin and the record at the top of its lockfile
  (`pnpm install --lockfile-only`), the workflows, the Makefiles' `PNPM` defaults and
  `PNPM_VERSION` with `PNPM_SHA512`, which the guard then holds everything to.
- CI fetches pnpm through the corepack Node 24 ships. Node 25 and later do not ship corepack, so a
  move of CI past Node 24 needs another way to fetch the pinned pnpm. The development shell
  provides pnpm itself.
