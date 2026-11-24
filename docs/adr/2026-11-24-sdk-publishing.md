# ADR: The SDK, `@dcl/protocol` and `@dcl/hashing` publish from dcl-one

- Status: Accepted
- Date: 2026-11-24

## Context

`library/js-sdk-toolchain` builds the creator SDK from `library/protocol` and `library/hashing`
([SDK](2026-10-26-sdk-in-dcl-one.md), [protocol](2026-10-25-protocol-in-dcl-one.md),
[hashing](2026-11-13-hashing-in-dcl-one.md)). Until now `sdk.yml` only packed it. The published
SDK came from `decentraland/js-sdk-toolchain`, and that SDK pinned a protocol and a hashing
implementation that are not the ones dcl-one ships. A packed `@dcl/sdk-commands` names
`@dcl/protocol` and `@dcl/hashing` by exact version, so the SDK cannot publish from here unless
both packages publish from here too.

## Decision

`.github/workflows/sdk.yml` publishes `@dcl/hashing`, `@dcl/protocol`, `@dcl/js-runtime`,
`@dcl/ecs`, `@dcl/react-ecs`, `@dcl/sdk-commands`, `@dcl/playground-assets` and `@dcl/sdk` with
npm trusted publishing (workflow `sdk.yml`, environment `sdk-release`, `main` only).

- **Channels.** The `channel` job picks one:
  - `latest` for a manual run with `release` set to `patch`, `minor` or `major`.
  - `latest` with a patch bump for a `main` push that changes the `@dcl/inspector` pin in
    `packages/@dcl/sdk-commands/package.json`. Merging an Inspector bump releases the SDK.
  - `next` for every other `main` push, as `<X.Y.Z+1>-<run id>.commit-<sha7>`.
- **Inspector bump.** When Creator Hub CI publishes `@dcl/inspector` on `main`,
  `.github/workflows/sdk-inspector-bump.yml` opens or refreshes the pull request on
  `bot/sdk-inspector-bump`. It pins that version in `@dcl/sdk-commands`, with the newest
  `@dcl/asset-packs` its range accepts (the dependency and the workspace override). A person
  reviews it, and merging it releases the SDK. A GitHub App opens it, because a pull request opened
  with the workflow token runs no checks. It reuses `nixpkgs-update.yml`'s app
  (`NIXPKGS_UPDATE_APP_ID`), installed here with contents and pull-request write access. The SDK
  workspace exempts both packages from `minimumReleaseAge`, since the bump runs minutes after they
  publish. A manual run pins any version, npm `latest` by default.
- **Version.** It counts from the highest `@dcl/sdk@X.Y.Z` tag. A `main` publish without one
  fails instead of starting from 0.0.0. A `latest` release tags `@dcl/sdk@X.Y.Z` with release notes
  for `library/js-sdk-toolchain`, `library/protocol` and `library/hashing` once every package is on
  npm. A failed run therefore computes the same version again, and it skips the packages npm
  already has.
- **One `latest` release at a time.** `latest` publishes share one concurrency group, so each
  computes its version from the tag the previous one made. A newer pending release replaces an
  older pending one; its Inspector pin already includes the older one's. `main` runs of `sdk.yml`
  are never grouped, so a later push cannot drop a pending Inspector release.
- **Protocol and hashing.** Each publishes once for each commit that changes its folder, as
  `<manifest version>-<run id>.commit-<sha7>` under the `next` dist-tag. A release reuses the
  snapshot npm already holds for that commit. The SDK pins these versions exactly through
  `make prepare`'s `PROTOCOL_VERSION` and `HASHING_VERSION`. `pnpm pack` drops `packageManager`,
  so the published packages carry nothing pnpm-specific. `@dcl/hashing` counts
  from 3.1.0, above every version `decentraland/hashing` published.
- **Order.** Hashing and protocol go first, then js-runtime, ecs, react-ecs, sdk-commands and
  playground-assets. `@dcl/sdk` goes last, so when it moves, every package it names is on npm. The
  tarballs published are the ones `scripts/check-no-local-packages.cjs` checked.
- **Dry run.** A manual run publishes with `pnpm publish --dry-run` unless it runs on `main` with
  `dry-run` off.
- **Pull request builds.** A pull request from a branch of this repository packs the same set at
  its snapshot version and publishes nothing to npm. Each `workspace:` dependency points at its
  sibling's tarball URL (`make prepare` with `PACKAGE_BASE_URL`), and so does a protocol or hashing
  snapshot npm does not hold yet. Each tarball goes to the SDK bucket in its package's folder,
  `@dcl/js-sdk-toolchain/branch/<branch>/@dcl/<package>/`. `playground-assets` goes to
  `@dcl/js-sdk-toolchain/branch/<branch>/playground-assets`, where the playground's `?sdk-branch=`
  reads it. The "SDK test builds" comment gives the install commands. The CLI e2e job runs
  `init`, `build` and `export-static` with those tarballs and uploads the realm to the bucket's
  `ipfs/`. `main` uploads nothing, since npm carries its packages. The upload role is
  `CREATOR_HUB_SDK_BUCKET_ROLE_ARN`, the same role as the Inspector's previews. As in the Creator
  Hub workflows, each step that needs the bucket, its region or its base URL reads that secret
  itself.
- **GitHub releases.** A package release in this repository (`@dcl/sdk`, `@dcl/inspector`,
  `@dcl/asset-packs`) is never a pre-release and never the repository's Latest. dcl-one releases
  several projects, npm carries the packages, and the Creator Hub's updater reads only
  `decentraland/creator-hub`.

## Consequences

- A proto change reaches creators with the next SDK snapshot on `main`, and in `latest` with the
  next release.
- `@dcl/protocol@next` and `@dcl/hashing@next` follow dcl-one. Their `latest` dist-tags stay where
  their former repositories left them.

## Cut-over

1. Point npm trusted publishing for the eight packages at `decentraland/dcl-one`, workflow
   `sdk.yml`, environment `sdk-release`. Create that environment, limited to `main`.
2. Let the pull request role behind `CREATOR_HUB_SDK_BUCKET_ROLE_ARN` write, set the ACL of and
   delete objects under `@dcl/js-sdk-toolchain/*` and `ipfs/*`, and list the bucket for the
   playground sync. It writes only under `creator-hub/` today.
3. Tag the last SDK release published from `decentraland/js-sdk-toolchain`: `@dcl/sdk@7.33.0`.
4. Stop publishing from `decentraland/js-sdk-toolchain`, `decentraland/protocol` and
   `decentraland/hashing`. Until then, both sides move the same `next` dist-tags and write the
   same bucket folders.
5. Add to the first `latest` release's notes the narrowed generated types listed in the
   [SDK decision](2026-10-26-sdk-in-dcl-one.md).
