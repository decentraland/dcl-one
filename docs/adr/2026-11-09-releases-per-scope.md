# ADR: Releases are cut per scope by a manual workflow, under a prefixed tag

- Status: Accepted
- Date: 2026-11-09

## Context

One repository holds deliverables that ship on their own cadence: the SDK packages, the protocol
package, the Explorer desktop and headless builds, the Catalyst images. GitHub releases are
repository-wide: one list, one "Latest" badge, and `--generate-notes` ranges over every merged
pull request. The Explorer workflows already cut their own releases from `workflow_dispatch`
under `bevy-explorer-desktop-v*` and `bevy-explorer-headless-v*` tags; the SDK had no release path
from here ([SDK ADR](2026-10-26-sdk-in-dcl-one.md), follow-up 1).

## Decision

- A scope releases through its own `workflow_dispatch` workflow, run from `main`. The run builds,
  tests, publishes, then creates the tag and the release itself with `gh release create`; a tag
  push triggers nothing. A scope's version either lives in the tree or is a dispatch input, as
  the scope's packaging dictates; a rerun of a released version stops before any write.
- Tags are `<scope>-v<version>`. Scope releases pass `--latest=false`; the "Latest" badge is kept
  for a future release of every scope at once. Release notes range over the scope's paths since
  the previous tag with the same prefix, not over the repository.
- Scope workflows also expose `workflow_call`, so one dispatcher can later release several scopes
  from a single run.
- The SDK (`.github/workflows/sdk-release.yml`) takes the version as an input, as upstream takes
  it from the release tag, and stamps it into the manifests before packing (the tree keeps a
  placeholder). It publishes `ecs`, `js-runtime`, `react-ecs`, `sdk-commands` and `sdk` in
  dependency order through npm trusted publishing (dist-tag `latest`, `rc` for a prerelease
  version) and releases as `sdk-v<version>` with the tarballs attached; `playground-assets` is
  built and tested but not published. The SDK embeds the protocol at its commit
  ([SDK ADR](2026-10-26-sdk-in-dcl-one.md)), so it needs no protocol release; the notes name the
  `library/protocol` commit it was generated from.

## Consequences

- Each SDK package needs trusted publishers naming this repository and both `sdk.yml`
  (`next` on `main` pushes) and `sdk-release.yml`; until then those runs fail at the first publish.
  npm checks the calling workflow's filename, so a dispatcher that calls `sdk-release.yml` needs
  its own entry.
- SDK and Explorer compatibility is by commit, not by a shared protocol version: a scene built
  with `@dcl/sdk` from a commit assumes the components `library/protocol` had there.
- The Explorer workflows keep their tag prefixes; moving them to `--latest=false` and the image
  workflows off bare `v*` tags follows this scheme.

## Follow-ups

1. A protocol release workflow publishing `@dcl/protocol` from `library/protocol` under
   `protocol-v<version>`, for the consumers outside this repository (Godot and Unity take the
   proto files, TypeScript services take `out-ts`); it needs a version scheme and the package made
   public.
2. A release-everything dispatcher that calls each scope's `workflow_call` and creates the one
   release that claims "Latest", listing the scope versions it shipped.
