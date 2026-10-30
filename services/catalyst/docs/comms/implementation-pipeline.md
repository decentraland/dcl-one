# Implementation pipeline: catalyst-comms

How agents build comms and multiplayer work ([comms-crate.md](comms-crate.md)) and where each role gates it.
Role definitions: [`.claude/agents/`](../../.claude/agents/). Reporting workflow: [README.md](README.md).

## Roles

| Role | Who | Does | Never |
|---|---|---|---|
| **Main session** | the user's driving session | plans work packages, dispatches every agent, merges reports into one fix list, records the user's decisions and verifies the combined evidence | writes implementation code; accepts a decision for the user |
| **Implementer** | one agent per work package, own worktree and branch | code and tests for its package; returns a report with an `architecture` block | changes a channel, crate boundary or decision on its own; touches another package's files |
| **Architect** | `comms-architect` | validates technologies, channels, abstractions and complexity; raises decisions for the user and change requests for implementers | writes implementation code; accepts its own proposals |
| **CI engineer** | `comms-ci-engineer` | turns rules into CI checks (catalog, tiers, supply-chain and release gates, flaky quarantine, toolchain); gives every PR revision a gate verdict | writes feature code or tests; decides a rule |
| **Code reviewer** | a fresh agent per PR, or `/code-review` | correctness and tests | sees the implementer's transcript |
| **Divergence analyst** | `divergence-analyst` | compatibility of every client- and user-facing API with each client version in use; writes its conformance check | judges architecture or security |
| **Security reviewer** | private `comms-security-reviewer` workflow | exploit paths in identity, tokens, admission, bans, relayed messages, rate limits and subjects | fixes code |
| **The user** | | accepts or rejects decisions, declines blocking change requests, approves merges | |

## Stages

1. **Design.** The main session updates the design doc; the architect and `divergence-analyst` run design-stage; the CI engineer plans `planned` checks. Gate: every decision the stage depends on is decided, no open `blocking` request, no open Critical divergence.
2. **Work breakdown.** Each package names the components, channels and abstractions it touches; parallel packages share no component; a channel has its schema and fixtures before either side starts. A CI `build` run sets up the required tier first. Gate: no `blocking` request; the required tier is green on the default branch.
3. **Implement.** One implementer per package. A reported deviation goes to an architect `design` run. Gate: the report lists the tests run, passing, and its `architecture` block.
4. **Review.** In parallel, each in fresh context: code reviewer, architect (when a trigger below applies), CI `gate` run, `divergence-analyst`, and `comms-security-reviewer` when the diff touches identity, tokens, admission, bans, relayed messages, rate limits or subjects.
5. **Fix loop.** The main session merges reports into one fix list; reviewers re-check at the new head. After 3 rounds, the user decides.
6. **Merge.** The user merges when the merge gate holds; the merge queue re-runs the required tier.
7. **Audit.** After every third merged package and before each release, the architect checks drift and complexity, the CI engineer tier times, flakes and advisories. Each finding becomes a change request, a doc edit or a `planned` check.
8. **Release.** A CI `release` run (SBOM, signatures, image scan, migration safety, rollback); the user approves the rollout.

## Architect triggers

Run the architect on every PR that adds or changes a crate, service, binary or process; a
`Cargo.lock` dependency, technology or fork; a route, message, NATS subject, room name,
control command or wire format; a trait, module layer, wrapper, generic or flag; an edge
between crates or services. The main session checks the diff itself, not only the report.

## Merge gate

- The CI verdict is `pass` for the head sha: every required check ran and passed, none the diff should trigger is missing, every flake is recorded.
- No open `blocking` change request; every `should` request is `done` or `declined` with a reason.
- Every decision the package relies on is `accepted`.
- No open Critical or High divergence without the user's decision; every changed client-facing API is `compatible` for each caller and its conformance check passes.
- No open security finding at High or above.
- The code reviewer approves.
- The main session has reconciled the architecture and divergence reports and verified their evidence at the target revision.

## Implementer report `architecture` block

```json
"architecture": {
  "added": [{ "unit": "crate | service | binary | channel | trait | module | flag | dependency", "id": "<name>", "why": "<one line>" }],
  "deviations": [{ "from": "<decision id or doc § section>", "what": "<one line>", "why": "<one line>" }],
  "change_requests": [{ "id": "<area>-change-<n>", "status": "done | declined", "note": "<commit, or the reason>" }]
}
```

## Loop rules

- Answer every change request: `done` with the commit, or `declined` with a reason. Only the user declines a `blocking` request.
- Decisions are the user's; the main session records the verdict and who gave it.
- Never negotiate design in an implementation: stop and report a deviation.
- Turn each lead for `comms-ci-engineer` into a `build` run: architect rules and budgets, each divergence golden capture and its schema-diff gate, each tool-visible security bug class. Once a check is active, take its result instead of re-reading the code.
- Show reviewers artefacts (diff, package, reports, CI results), never transcripts.
- Return every changed record ID and its evidence; the main session reconciles the reports and verifies their evidence per `README.md`.

The security reviewer is configured in the authorized private review workspace. When a required review is unavailable, report that gate as pending; its absence is not approval.
