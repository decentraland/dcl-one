# ADR: CI publishes the Explorer's patched sources after the merge

- Status: Accepted
- Date: 2026-11-17
- Supersedes in part: [The Explorer's patched sources are git commits in this repository](2026-11-15-explorer-patched-sources-as-git-commits.md)

## Context

Cargo takes each patched source as a parentless commit of this repository, by `rev`. Only an
account with push rights could write `refs/patched-sources/<name>/<rev>`, so a PR from a fork that
changed a patch series failed its Cargo jobs until a maintainer ran `publish` on a checkout of it.

GitHub serves through a repository, by id, the commits of each of its forks. `git fetch` and
Cargo's default fetch both take a commit that only a fork holds from the parent's URL.

GitHub refuses a push of a commit that holds a file under `.github/workflows/` from a token
without the `workflows` permission, on any ref, `refs/patched-sources/*` included. A workflow's own
token cannot have that permission, and a contributor's token may lack the `workflow` scope. 14 of
the 18 patched trees held such files.

## Decision

- **Before the merge:** the contributor pushes each new `rev` to their own fork with
  `publish --remote https://github.com/<you>/dcl-one`. Cargo fetches it from this repository's URL,
  in CI and locally. Nothing changes in `Cargo.toml` or `Cargo.lock`.
- **PR check:** `publish --check --fetchable` in the job that every build job needs. A `rev` passes
  when this repository has its ref, or when this repository serves the commit by id. It fetches the
  commit alone, without its tree.
- **After the merge:** the `bevy-explorer-publish-sources` workflow runs on each push to `main`
  that changes `rendering/bevy-explorer/patches/`, with `contents: write`. It runs `publish` with
  the tool and series of `main`. It has no concurrency group: a group keeps one pending run and
  cancels the others, and a cancelled run would leave the `rev` of its push without a ref. Runs may
  overlap, since a ref that another run already pushed is up to date. A manual run on `main`
  repairs a failed one.
  `publish` rebuilds each commit that this repository lacks from its upstream commit and series,
  fails when the result is not the `rev`, and pushes the ref. It never trusts an object of the fork.
- **Daily:** the drift workflow keeps `publish --check` without `--fetchable`, so a `rev` without a
  ref fails it.
- **No workflow files:** each source whose upstream has a `.github` directory lists it in
  `exclude`. Cargo reads nothing there. The tool fails a tree that holds a file under
  `.github/workflows/`, whether the base or a patch brings it. Excluding `.github` changed the
  `rev` of those 14 sources once; the refs of their earlier revisions stay. `nix_hash` covers the
  whole upstream commit, so it did not change.

## Superseded in the 2026-11-15 ADR

- Decision, "Publish": a workflow runs it on each push to `main` that changes a source; a manual
  run of that workflow repairs a failed run.
- Decision, "Proof", `publish --check` in the job of `pins`: it runs with `--fetchable` there.
- Consequences, "Publishing is manual".

## Rejected options

- **Publish from the PR with `pull_request_target`:** a job with a write token would read a fork's
  `source.toml` and patches, and fetch the upstream it names. The refs of PRs that never merge
  would stay, since a ref is never deleted.
- **Fill Cargo's git cache in CI from `verify`:** it needs no fork, but CI would depend on Cargo's
  cache layout, and a contributor's local build would still fail to fetch the `rev`.
- **A GitHub App token with the `workflows` permission, as a secret:** the trees could keep
  `.github`, but it needs an App installed on this repository, and a stored credential that can
  change this repository's workflows. Contributors without the `workflow` scope still could not
  push to their forks.

## Consequences

- A fork PR builds without a maintainer. Its contributor runs one more command, which the PR
  check names when it fails.
- Before the merge the build depends on the fork. A deleted fork, or a fork whose objects GitHub has
  pruned, fails the PR check until the contributor pushes again. After the merge the ref keeps the
  commit.
- `main`'s own build jobs on the merge push still fetch through the fork while the publish
  workflow runs.
- A push publishes the pins of its last commit only. A squash merge pushes one commit; a push of
  several commits that each change a `rev` leaves the earlier ones without a ref, which the daily
  check does not see, since it reads the current pins.
- The token of the publish workflow can push any ref of this repository. It runs only on `main`,
  with code that review merged.
- The PR check fails with `cannot check` when the fetch gets no answer (a network failure or a
  60-second timeout); a rerun, not a push to the fork, is the remedy then.
