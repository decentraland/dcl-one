# Contributing

Smaller PRs are better. Everything below is in service of that and of a reviewer who can
understand the change in one sitting.

## Scope

- One problem per PR, fixed at the sentence that names it. If the fix wants new state or
  plumbing the problem didn't mention, that is a second PR and a conversation first.
- Anything that changes an interface between areas, adds a scene-facing capability, or will
  take more than a couple of days gets a short written plan before a branch exists. The
  approach, not prose.
- For multi-step features, get the whole pipeline working as a proof of concept first, then
  split it into reviewable PRs. Intermediate states that build but cannot be exercised are not
  reviewable.
- We squash-merge, so stacked PRs conflict on the child after the base lands. Prefer
  sequential small PRs over stacks.

## The diff

- Nothing in the diff the change doesn't require. No reordering, restyling, renaming locals or
  tidying neighbours. Reviewer load beats marginal tidiness.
- Comments say what the code cannot: a current constraint, a non-obvious invariant, a platform
  quirk. Not what the code does, and no archaeological commentary: not what the old design was,
  not why the new one doesn't need X, not an argument addressed to the reviewer. Strip those
  before pushing. Rationale goes in the PR description.
- Existing comments survive a refactor unless they became wrong.
- Reuse what exists: protocol and SDK types, the existing scheduling, picking and loading
  paths. Hand-rolled parallels are the main source of drift.
- You should not need to add a schedule set or ordering step. If you think you do, raise it
  first; the [scheduling guide](scheduling.md) explains the current setup.
- If something works better with a change in a dependency, change the dependency. Upstream it
  where possible; where it is bespoke to us, we maintain our forks for exactly that. See
  [the bevy fork](bevy-fork.md).

## Tests

- Test behaviour at a boundary, not the shape of the implementation. A test that has to change
  when the code is refactored without a behaviour change is a cost, not an asset.
- Parse structured output; never string-compare it. Serialisation order is not stable across
  feature unification.

## The PR description is the record

- Squash-merge discards commit bodies. The description carries the why, the alternatives
  rejected, and how it was verified, naming the platforms covered: native, web, headless.
- A diagram for anything with dataflow, ordering or lifetime changes. A draw.io SVG or a
  mermaid block both render on GitHub. A reviewer reads a diagram in a minute and a 400-line
  diff in an hour.
- Screenshots before and after for rendering changes. There is no visual regression suite for
  the 3D side.
- Follow the repository PR template, including its test instructions and checks run; omit a chronological work narrative.

## Before pushing

- Merge main first. CI tests the merge, not the branch.
- `cargo fmt --all`, `cargo clippy --locked -- -D warnings`, `cargo test --all`. Clippy warnings
  are errors in CI.
- The wasm build, only if the change has platform-sensitive code: `#[cfg]` branches,
  threading, atomics, blocking calls, timers, filesystem or network access, wasm-specific deps.
- Web-only engine switches are a page param plus a wasm export, not a launch option.

## Dependencies

- **Never run `cargo update`. Edit `Cargo.lock` by hand.** Cargo's re-resolve is not
  deterministic for this workspace: any `cargo update -p <crate>`, and any dependabot rebase,
  also flips unrelated lines (`syn` 2 to 1 under `data-encoding-macro-internal`, `heck` 0.5 to
  0.4 under `prost-build`, and others). Those flips are rejected in review even when CI is
  green. To bump a dependency, change its `version`, `source` or git hash, `checksum` and
  dependency list in the lock directly, then validate with `cargo metadata --locked` (exit 0
  means the lock was accepted unchanged). For a dependabot PR, take main's lock and hand-apply
  only the intended crate's change.
- Hoist every third-party dependency to `[workspace.dependencies]` and use
  `{ workspace = true }` at the call site, even for a crate used in one place. Convert existing
  inline uses of the same crate in the same change.
- Changes to bevy go in [the fork](bevy-fork.md); that page covers patching it locally.
- `rendering/bevy-explorer/forks.toml` and `cargo-patches.toml` are generated from the lock.
  After changing a git dependency's revision, run `python3 tooling/upstream-forks.py` from the
  dcl-one root so they match; a daily workflow otherwise opens an `upstream/forks` PR.

## Review

- First response within one working day. If it hasn't come, ping the reviewer on Slack rather
  than wait.
