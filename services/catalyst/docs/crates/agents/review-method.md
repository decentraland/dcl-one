# Crate distribution review: method

Review what each crate owns, whether the split is adequate and what to change. Read the target revision and current architecture records before judging it. Return proposed per-area records using the fields below; the main session reconciles them under the [reporting workflow](../../comms/README.md). Working reports stay in root `.agent-work/`.

- **Pin:** resolve the target revision before reading; never reuse a historical review's verdict without checking it.
- **Paths:** relative to `services/catalyst/` unless explicitly repository-root paths.
- **Scope:** comms and multiplayer have their own architecture workflow in `../../comms/`; take the user's accepted decisions as constraints.

## Areas

| Area | Crates |
|---|---|
| `worlds` | catalyst-worlds |
| `social` | catalyst-social |
| `content` | catalyst-content, catalyst-storage, catalyst-chain-index, catalyst-land-authz |
| `explore` | catalyst-explore |
| `shop` | catalyst-shop, catalyst-price |
| `builder-creator-sdk` | catalyst-builder, the scene SDK crates identified by the target workspace manifests |
| `camera-reel-quests-governance` | catalyst-camera-reel, catalyst-quests, catalyst-governance |
| `shared-backend` | catalyst-http, catalyst-db, catalyst-auth, catalyst-jobs, catalyst-fed, catalyst-shared, catalyst-testing, catalyst-config, and the root crate `src/` (ops, presence, serve, index, main) |

## Contracts every proposal keeps

From the root `AGENTS.md`, `README.md`, `DATABASE.md` and the comms decisions:

- Layers follow `docs/module-layers.toml`.
- Dependencies never point upward; peers on one layer do not use each other.
- A back-edge is designed away with a trait the lower layer owns; it is never added to `[allow]`.
- A new crate needs an independent consumer or a concrete build requirement (e.g. dev-loop compile time in the measurement record); size alone is not a reason.
- One PostgreSQL database, one fresh-schema bootstrap; existing databases are never reset to fit a schema; atomic workflows use direct SQL joins and a passed transaction.
- No internal HTTP hops and no per-service pools.
- HTTP paths, room names, authorization and crash/resume contracts are preserved when code moves.
- A change lands with everything it touches; internal contracts get no compatibility shims, only client-facing contracts stay compatible.
- Comms placement follows root `AGENTS.md`, `docs/module-layers.toml` and `docs/comms/comms-crate.md`. Read the current decisions before proposing changes; older review snapshots are not a separate ownership authority.

## What to judge

Per module (roughly each directory or large file under `src/`): **job** (one line); **upstream** Decentraland service it replaces (from the current compatibility/readiness records and crate docs); **fit**; **proposed home** (`stays` or crate::module).

Fit is one of: `Fits`; `Misplaced` (belongs to another area or crate); `Duplicated` (same job elsewhere); `Oversized` (several jobs to split inside the crate); `Retiring` (replaced by catalyst-comms or going away); `Thin` (too small for its own crate or module: merge); `Unclear` (overlapping or undocumented job).

Per area: cohesion (crate description, `AGENTS.md` Ownership and code agree; name modules doing another area's job); coupling (module edges derived from the pinned manifests and imports, plus cross-domain SQL found by grepping SQL strings for schema names; real vs accidental edges); layer position (too high blocks peers, too low forces callbacks); duplication; size and shape (splits need the new-crate rule); stubs and gaps that affect distribution (501 routes, empty modules, dead code); tests and odd cross-area `test-support` features; whether `ownership.json` matches crate boundaries.

This is not a security audit. Return security leads only through the authorized private review workflow; never copy them into public files or PR text.

## Proposals

Each must be decidable in one sitting: the concrete change (modules from/to, approximate lines); the reason, citing findings; cost (S under 2k lines touched, M 2k to 10k, L over 10k), risk and what it unblocks; alternatives it beats and why; dependencies (a comms decision, another proposal); and a recommendation: `Recommend`, `Consider` (real trade-off) or `Defer` (say what would trigger it).

Record "keep as is" outcomes with the reason, so the review is not reopened. Prefer few strong proposals. A proposal moving code into another area names that area; that area's page picks it up at reconciliation.

## Per-area report schema

```json
{
  "area": "worlds",
  "title": "Worlds",
  "crates": ["catalyst-worlds"],
  "lede": "One or two sentences: what the area owns and the verdict in plain words.",
  "verdict": {"label": "Adequate | Adequate with fixes | Needs restructuring", "summary": "Two or three sentences."},
  "facts": [{"label": "Lines", "value": "78,989"}, {"label": "Layer", "value": "8"}],
  "today": [{"id": "core", "name": "worlds::core", "note": "short job · 11.6k lines", "kind": "module | retiring | external"}],
  "proposed": [{"id": "worlds", "name": "catalyst-worlds", "notes": ["one short line per job"], "kind": "crate | new | other | comms | removed"}],
  "edges": [["core", "worlds", "main | faint"]],
  "modules": [{"module": "worlds::core", "lines": 11622, "job": "…", "upstream": "worlds-content-server", "fit": "Fits", "proposed": "stays", "proposal": "worlds-proposal-1"}],
  "findings": [{"id": "worlds-finding-1", "title": "…", "detail": "One to three sentences.", "evidence": ["crates/catalyst-worlds/src/core/mod.rs:42"], "weight": "High | Medium | Low"}],
  "proposals": [{"id": "worlds-proposal-1", "title": "…", "why": "…", "what": "…", "cost": "M · about 6k lines move", "risk": "…", "unblocks": "…", "alternatives": "…", "depends_on": ["<accepted-decision-id>"], "findings": ["worlds-finding-1"], "recommendation": "Recommend", "cross_area": ["social"]}],
  "keep_as_is": [{"title": "…", "why": "…"}],
  "open": [{"title": "…", "detail": "The question, the options and the one you lean to."}]
}
```

- `today`: the area's current modules; group small ones, at most 14 blocks.
- `proposed`: where they end up; at most 8 blocks.
- `edges` link them. Proposed `kind` drives the colour: `crate` the area's crate after the change, `new` a crate the proposals create, `other` another area's crate, `comms` catalyst-comms, `removed` deleted.
- Every finding cites `path:line` in `evidence`. Keep cells short: the page is a briefing.

Use readable codes in full words. Never use absolute or home paths, machine names, private addresses or email addresses: the tree is public. Review the diff for these values before publishing; do not assume a sanitization check is installed.
