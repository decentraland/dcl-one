# ADR: The Explorer's patched sources are git commits in this repository

- Status: Accepted; superseded in part by [2026-11-17](2026-11-17-explorer-patched-sources-published-by-ci.md)
- Date: 2026-11-15
- Supersedes in part: [The Explorer's Bevy and forked crates are an upstream commit plus patches](2026-11-14-explorer-sources-as-patches.md)

## Context

The Explorer builds 18 patched sources: Bevy and 17 forks. Each one is an upstream commit and a
patch series. Cargo read each one as a path dependency, from a tree that a tool rebuilt in the
worktree.

- Cargo compiles a path dependency for each worktree, and these compiles miss sccache. A second
  worktree compiled all 18 sources again.
- Every build ran a step before Cargo and needed a guard against a stale tree.

## Decision

The patch series stays the tracked form. Cargo takes each patched source as a git commit of this
repository.

- **Commit:** it has no parent and a fixed author, time and message. Its tree is the patched
  source. The id follows from the upstream commit and the patch series alone. `source.toml`
  records it as `rev`.
- **Refs:** this repository holds each commit at `refs/patched-sources/<name>/<rev>`. A clone does
  not fetch these refs.
- **Cargo:** `rendering/bevy-explorer/source/Cargo.toml` and `Cargo.lock` name each source as
  `git` + `rev`. Cargo fetches only those commits. `cargo build --locked` works on a fresh
  checkout with no step before it.
- **Edit:** `edit NAME` writes the tree at `rendering/bevy-explorer/bevy` or `forks/<name>` and
  makes both Cargo files read that path. `export NAME -m "subject"` writes the next patch, the new
  `tree` and `rev`, and names the new `rev` in both Cargo files. `edit NAME --stop` names the
  `rev` again without a patch. `repin NAME` writes `tree` and `rev` and names the `rev` in both
  Cargo files from any earlier state, for a new upstream commit or a series changed by hand.
- **Publish:** `publish` pushes the commits that the repository does not have. It needs push
  rights, so a maintainer runs it, with the tool of `main` on a checkout of the PR. No workflow
  runs it.
- **Proof:** the id check is the proof. The repository URL is not: through it GitHub serves a
  commit of any fork by id.
  - `pins` runs offline in CI, in the job that every build job needs. Each `rev` in the manifest
    and the lock must be the one in `source.toml`. Each other git source must be on the tool's
    list. It compares repository URLs without case, `.git` or a last `/`. It fails while a source
    is in edit mode.
  - `verify` rebuilds a source from upstream and proves `tree` and `rev`. CI runs it on each
    source that a PR or a push to `main` changes, before the jobs that publish. A daily run
    covers all sources.
  - `publish --check` runs in the same job as `pins`, and daily. It proves that each pinned `rev`
    has a ref.
  - `materialise` fails when the tree it wrote is not `rev`.
- **Nix:** the Nix package does not fetch the commits. It fetches each upstream commit by hash,
  applies the patch series with the tool, fails when the result is not `rev`, and gives Cargo the
  crates as a vendor directory.
- **In-tree crates:** the crates under `rendering/bevy-explorer/vendor` stay path dependencies.

The commands are in [the bevy fork](../../rendering/bevy-explorer/docs/guidelines/bevy-fork.md#changing-bevy).

## Superseded in the 2026-11-14 ADR

- Decision, "Cargo" and "Stale trees".
- Decision, "CI cache key": the key holds the in-tree crates only. `Cargo.lock` names the
  revisions.
- Decision, "Rebuilt": Cargo reads a tree only while its source is in edit mode, and in the Bevy
  crate tests, which run in the tree that `materialise bevy` writes.
- Reasons against patch files, first bullet: no patch is applied before Cargo resolves.
- Verification, second and fourth bullet: a locked resolve also fetches the 18 commits, and a
  fresh checkout rebuilds no tree.
- Consequences, the bullets on the first fetch of the upstream commits, the stale-tree check, the
  build with no rebuilt tree, and the warnings of the forked sources.
- Consequences, first bullet: a change starts with `edit`, and the PR also changes `rev`.
- Consequences, the bullet on an upgrade: an upgrade also runs `repin`.

## Rejected options

- **A second repository for the commits:** a build needs one more repository, which the
  2026-11-14 decision removed.
- **Tags:** a clone fetches every tag, so each clone gets every revision of every source.

## Consequences

- A build needs objects of this repository that are on no branch. Both Cargo files name this
  repository by URL, so a fork or a mirror also fetches the commits from here, and a build with
  no access to this repository needs a Cargo git cache that already holds them.
- No repository rule protects the refs. An account with write access can delete one. Cargo
  fetches a commit by id, so a changed ref cannot change what a build compiles, and
  `publish --check` fails when a ref is missing.
- The refs must stay. An older revision of this repository builds only while its refs exist.
  `publish` never deletes a ref and never forces one.
- Publishing is manual. The Cargo jobs of a PR that changes a patch series fail until a
  maintainer runs `publish`.
- Two worktrees share the compiled sources through sccache.
- The 17 patches that only allowed rustc warnings are removed: Cargo caps the warnings of a git
  dependency. A source in edit mode shows its warnings.
- `pins` fails in edit mode, so an edit cannot be merged by accident.
