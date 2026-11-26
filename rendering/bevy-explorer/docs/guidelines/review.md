# Reviewing engine changes

These criteria refer to core engine code: the Rust workspace under `source/` (`crates/`, `src/`),
the engine's own page and scripts under `source/deploy/web/`, and the bevy and forked-crate
patches under `patches/`. The web HUD under `source/react-web/` is reviewed against
[`react-web/review.md`](../../source/react-web/review.md) and its `AGENTS.md` instead.

Each rule below names the guideline it rests on. Read the guidelines from the head of `main`,
not from the PR branch: a branch cut before a rule landed is still reviewed against it.

## What a review covers

1. **Intent and breadth.** Say briefly what the PR does, how far it reaches, and why the design
   fits before listing what it should change.
2. **One PR.** Is this one problem, fixed at the sentence that names it? See R1.
3. **Correctness.** Bugs, races, leaks, ordering, platform behaviour. For every value whose
   behaviour the PR changes (a diagnostic, component, event, stream or setting), list everything
   in the workspace that reads it, not only the readers in the diff, and check that each still
   holds.
4. **The rules on this page.** Work through every rule against the diff; do not sample. A rule
   with no hit is not reported.
5. **Consolidation.** Check every new path against what already exists. See R3.
6. **Structural changes.** Always include a section listing each top-level structural change and
   whether the PR justifies it. See R17.
7. **Earlier review rounds.** On a re-review, check that each earlier finding, the bot's included,
   was fixed or answered. One left without either is a finding at its original severity. Then
   review the whole PR again, not only the commits since the last round.

The description's account of where the risk lies is a claim to check, not the bounds of the
review. State what was built or run and what was only read. Leave mergeability checks and merge-conflict
enforcement to GitHub. Continue reviewing the diff. Mention a merge conflict only to explain
why a specific verification step could not be completed.

Each finding gives the claim, `file:line`, the mechanism, a concrete fix and the rule it breaks.
A PR over 100 files outgrows the GitHub file list; diff a fetched head against its merge base instead.

## Severity

| Level | Bar | Examples |
|---|---|---|
| **P0** | A scene, a server or a remote peer can crash the client, escape the security model, or break the build on a platform | `unwrap` on scene data, a privilege check in JS, `Path::join` with content-server input, a wasm-only compile error |
| **P1** | A correctness bug, a design problem, an ambiguous fix, or a rule below marked P1 | Parallel plumbing beside an existing path, state in a side map, a new schedule set, an unexplained `Cargo.lock` flip, per-entity work every frame, unfiltered and unmeasured |
| **P2** | Small, unambiguous and low-impact | A comment that narrates the code, `Option<Res<T>>`, a stale line in the description |

P2 findings are notes: the PR can merge without them. Word them as suggestions, not as
conditions for merging. A problem the PR did not introduce is a follow-up issue, not a finding
against the PR.

## Rules

### Scope and the diff

**R1. One problem per PR · P1.** The fix answers the problem as stated. New state, signals or
plumbing the problem didn't mention, independently useful diagnostics, tooling, and changes to
unrelated behaviour are separate PRs. One feature delivered end to end is one problem, and
tooling that measured the change may stay with it. Intermediate states that build but cannot be
exercised are not reviewable on their own. [Scope](contributing.md#scope)

**R2. Nothing the change doesn't require · P2.** No reordering, restyling, renamed locals,
tidied neighbours or unrelated `rustfmt` reflows. Remove what no longer changes behaviour:
unreachable branches, duplicate tests, tests that assert nothing, and edits left over from an
abandoned design. Comments state a current constraint or invariant, not what the code does, not
what the old design was and not an argument to the reviewer. Existing comments survive a
refactor unless they became wrong. Raise to P1 when the noise hides the change.
[The diff](contributing.md#the-diff)

**R3. Reuse what exists · P1.** Every new type, enum, resource, queue, channel, observer, console
command, stream, rule, state copy or wire type is checked against what the workspace already
has: protocol and SDK types, the scheduling, picking and loading paths, helpers such as
`scene_runner::parcel_to_vec3`. A parallel system, rule or route beside an existing one is a
finding whatever the PR's size; ask for it to be folded into the existing path. A priority pick,
merge or sort over entities composes with the system that already does it
(`ActivePlayerComponent`). A small helper repeated where no shared one exists yet is P2.
[The diff](contributing.md#the-diff), [explicit updates](code-style.md#explicit-updates-over-clever-ordering)

### Security and untrusted input

**R4. The security model · P0.** Apply every rule in [security](security.md#rules). In
particular: a new scene-facing capability goes over `SystemApi`, never `RpcCalls`, and no JS
allowlist or op filter stands in for a Rust check; ask what stops the op in Rust if the scene
reaches it directly. Scenes reach the engine only through `SystemApi` and `RpcCalls`; a new
route between them is a finding. No path hands a scene, the sidecar or the app page anything
derived from the signing key. External components are joined with `join_relative`, never
`Path::join`. A capability the scene server can never use is excluded from the headless build
in code. Security paths fail closed.

**R5. Untrusted input is bounded · P0 / P1.** Scene data, content-server responses and peer
messages never take the client down: code handling them warns and skips, with no `unwrap` or
`panic` (P0). Ask what a scene author can make happen: unbounded growth, allocation outside an
asset loader, an engine-thread stall on web, identifiers sent to a third-party realm (P1).
`dcl_assert!` is for engine invariants. New `SceneMeta` fields parse leniently.
[Errors](code-style.md#errors-and-invariants)

**R6. Network capabilities and instrumentation · P1.** Changes to service routing, scene
network capabilities or request instrumentation carry the tests in
[tests](contributing.md#tests) and the description in
[the PR description](contributing.md#the-pr-description-is-the-record).
[Network capabilities](security.md#network-capabilities)

### The ECS

**R7. State lives on the entity it describes · P1.** A fact about an entity is a component on
it: not a `HashMap<Entity, T>` or `HashSet<Entity>` in a resource, not a parallel `Vec`, not a
`Local`. A second copy of a fact that already lives somewhere is a finding. A boolean that
systems branch on is usually a marker component. Proto `Pb*` types become engine components
through `TryFrom`; never redeclare a protocol or SDK type.
[State](code-style.md#state-lives-on-the-entity-it-describes)

**R8. Cleanup is structural · P1.** Spawned entities hang off their owner or carry
`DespawnWith(owner)`; never despawn everything and respawn each frame. Scene UI links, it
doesn't parent. Nothing a scene owns, such as a channel end, is kept in a `Local`.
[Cleanup](code-style.md#cleanup-is-structural)

**R9. Plugins and systems · P1 / P2.** A feature's resources, systems and debug section belong
to one `Plugin`, not a free `register(app)` and not spread over other plugins (P1). An operation
that refuses checks before it changes anything (P1). `Res<T>` over `Option<Res<T>>`, membership
checks as `Query<(), …>` and `.contains`, long per-item loops moved into a `SystemParam`
method, and `bevy::platform::time::Instant` are P2. A new or changed runtime feature exposes the
state and controls a tester needs in its plugin's
[debug panel section](recipes.md#a-plugin-section-in-the-runtime-debug-panel) (P2).
[Plugins and systems](code-style.md#plugins-and-systems)

**R10. Scheduling · P1.** A system in `Update` with ordering constraints goes inside one of the
`SceneSets` (`.in_set(SceneSets::PostLoop)`), never outside them ordered relative to one
(`.after(SceneSets::RunLoop)`). A free-standing system with no ordering constraints may sit
outside all of them. Within a set, order directly against another area's system with
`.before` or `.after` only when strictly necessary, and look first for a construction that
avoids the ordering. A PR should not need a new schedule set or ordering step; one that adds
one says why. A `PostUpdate` system that needs a position goes in or after a
`PostUpdateSets` variant, and a missing ordering point is a new variant in that chain, never a
standalone `SystemSet` in a feature crate. A system writing UI `Node` fields from world-space
data also orders `.before(UiSystem::Layout)`. [The frame](scheduling.md#the-frame),
[PostUpdate](scheduling.md#postupdate)

**R11. Explicit updates over clever ordering · P1.** When bevy's own system has already
consumed an event this frame, set the affected state directly at the point of change. Running a
second copy of a bevy system, or reordering schedules to run ahead of it, is a finding.
[Explicit updates](code-style.md#explicit-updates-over-clever-ordering)

**R12. Engine contracts are typed · P1.** The HUD and system scenes reach the engine through
typed `SystemApi` variants; console commands are for debugging and automation, not a contract.
A stream pushes on change rather than being polled on a timer. Feedback reuses state the engine
already exposes before a new route is added. [Security](security.md#rules),
[a new stream](recipes.md#a-new-engine-to-system-scene-stream)

### Async, errors and measurement

**R13. Async work · P1.** IO runs on `IoTaskPool` and its result lands in a component; systems
never block. The worker that knows the outcome reports it per item, in request order, with a
named error enum; the caller does not re-derive failures. Cancellation tokens pass through
outer layers unchanged, and the `select!` sits at the one await that can hang. Anything the
scene loader reads is an asset load. [Async work](code-style.md#async-work)

**R14. `Drop` · P1.** One blocking operation per `Drop`. A lock is released before another is
taken or file IO starts; multi-step cleanup has an explicit completion path.
[The diff](contributing.md#the-diff)

**R15. Significant frame cost is filtered or measured · P1.** Per-frame work that might be
significant, such as work that scales with the number of entities, scenes or assets, is either
filtered so it runs only when its inputs change (`Changed<T>`, `Added<T>`, a run condition), or
comes with a measurement of the workload and its effect on unrelated callers: `main` against the
PR, like for like (release builds, no profiler attached, the same wasm optimisation). A small
system doing a fixed, cheap amount of work needs neither. A new shared budget is always
measured. A budget
is a share of the target frame, falling back to 60 fps when uncapped, never a fixed number of
milliseconds. A change that limits per-frame work reports a manual web run. Costs that only an
opt-in tool pays are exempt. [Before pushing](contributing.md#before-pushing),
[shared budgets](contributing.md#reviewing-content-request-changes)

**R16. Errors, logging and diagnostics · P2.** `anyhow` at boundaries, a named error enum where
a caller branches on the kind. A flag that filters one `match` arm is a guard inside that arm.
Log levels follow [logging](code-style.md#logging). A measurement is a Bevy diagnostic, not a
log line or a static. Filter directives use the shortest covering prefix.

### Platforms, dependencies and structure

**R17. Structural changes are listed and justified · P1.** Name every `Cargo.lock` change (new,
removed or bumped packages, not only the intended one), every new, removed or changed cargo
feature, every new crate or workspace member, every dependency, every patch entry and every new
public module or item, and say whether the PR justifies it. An unexplained one is a finding.

- The lock is edited by hand. Unrelated version flips from a re-resolve are rejected even when
  CI is green. A dependency nothing uses is removed with its lock entry.
- Third-party dependencies are hoisted to `[workspace.dependencies]`.
- A public item with no caller outside its crate is private.
- No `test-support` or other test-only cargo features in this workspace: a test-only feature can
  change production types or paths under feature unification. Shared test setup goes in a
  dev-only crate, as `pipeline_test` does. The root `AGENTS.md` rule asking for `test-support`
  features does not apply here.

[Dependencies](contributing.md#dependencies)

**R18. Forked and vendored code is changed in place · P1.** A change to bevy or a forked crate is
a patch in `patches/` and a new `rev`, not a copy of the forked code inside the engine. When the
fix exists upstream, port it. [The bevy fork](bevy-fork.md#changing-bevy)

**R19. Platform boundaries · P1.** Platform-specific code is a leaf behind `platform` or a
`cfg` next to its only caller; shared logic never sees a `cfg`. Web-only engine switches are a
page param plus a wasm export, not a launch option. Check new functionality against native, web
and headless, and the wasm build for anything platform-sensitive.
[Platforms](platforms.md#keep-the-boundary-clean), [before pushing](contributing.md#before-pushing)

### Tests and the description

**R20. Tests · P1.** Tests the PR adds, and tests the guidelines require, follow
[tests](contributing.md#tests): behaviour at a boundary rather than implementation shape,
structured output parsed rather than string-compared, staged and animated changes tested
against each event that can arrive mid-way, and each required regression run by a CI job.
Production code and types do not change to suit a test: no `cfg(test)` substitute for a shipped
predicate. A test does not grep shader source or check a Rust copy of a shader formula; a
shader change has a [pipeline-compile test](recipes.md#a-pipeline-compile-test-for-a-shader).
Beyond those, a missing test is not a finding.

**R21. The PR description is the record · P2.** Squash-merge keeps only the description, so it
matches the final code: the why, the rejected alternatives, what was run and on which platforms,
and what was not. No stale stacking notes, unfilled template sections or claims the code doesn't
back. Dataflow, ordering or lifetime changes have a diagram, and measurements were taken at the
head, merged with `main`. A rendering change without before and after screenshots is P1.
[The PR description](contributing.md#the-pr-description-is-the-record)

**R22. Ports · P2.** A port names the files it follows, the constants carried over and every
divergence with its reason in the description; a changed number or range is a divergence. A
divergence worth keeping goes in [decisions](decisions.md) in the same PR.
[Porting](porting-from-unity.md)

**R23. Public prose · P2.** Code comments, tracked docs and the PR text follow
[public prose](contributing.md#public-prose), including its search for implementation
provenance. Docs and decision records state the rule and its reason, not the history of the
change.

## Not findings

- **Documented choices.** A behaviour recorded in [decisions](decisions.md) or
  [oddities](oddities.md) is deliberate. Read both before flagging something surprising.
- **Stream reply senders in a `Local`.** A system that keeps `RpcStreamSender` reply ends for
  `SystemApi` streams in a `Local` is the established pattern, and R8 does not apply to it. Other
  channel ends a scene hands over still fall under R8.
- **Client names where the guidelines allow them.** Compatibility notes, decisions, oddities
  and porting PRs may name other clients; see [public prose](contributing.md#public-prose).
- **Pre-existing problems.** Something the PR did not introduce or make worse goes to a
  follow-up issue.
- **Unrelated CI failures.** A failure in a job the diff cannot reach is a flake to re-run, not a
  finding.
- **Style that `rustfmt` and `clippy` settle.** CI enforces both.
