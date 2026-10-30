# Divergence records: schema

These fields describe the Notion divergence records and proposed updates. The main session saves and verifies them per [the record workflow](../../README.md). They are not repository files. Validate the field and status requirements before saving.

**General rules**
- Dates are `YYYY-MM-DD`. Text fields may carry `` `code` `` spans.
- An empty value is `""` or `[]`; never leave a field out.

## Contract catalog: the catalog

`{ "updated", "contracts": [ ... ] }`, each contract:
- `id` `<area>.<name>` (e.g. `gatekeeper.get-scene-adapter`), never changed once used. `area` `pulse|connector|gatekeeper|voice|streaming|presence|scene-host|rooms|bus`.
- `kind` `route|message|subject|room-name|token-claim|event|job|timing|limit|metric`; `summary` one line.
- `baseline` `{ "repo": "decentraland/<name>", "sha", "where": "<path:line>" }`; `shape` (fields, types, status codes, error bodies); `behaviour` (ordering, retries, timeouts, idempotency, fail mode, casing, units, limits, reconnect/kick/ban effects).
- `clients` `[ { "client", "where" } ]`, client one of `unity-explorer`, `godot-explorer`, `bevy-explorer`, `dcl-one-engine`, `sdk7-scenes`, `sdk-multiplayer-server`, `livekit-bots`. `consumers` `[ { "service", "where" } ]`. `searched`: the searches run when `clients` or `consumers` is empty.
- `upcoming` `{ "branch": "<repo>:<branch>", "change" }` or `{}`.
- `verdict` `same|shape differs|behaviour differs|absent|unspecified|not reviewed`; `findings` citing ids; `verified` date last re-read at its pin; `moved` newest first `[ { "from", "to", "change" } ]`.
- `exposure` `client|user|service|internal` (per the agent's *Client- and user-facing API*); `client` and `user` rows are the facing API.
- `compat` (facing rows; `[]` otherwise): one entry per caller and version range `[ { "caller", "versions" (e.g. `v0.185.0-alpha to dev`), "verdict": "compatible|breaks|unproven", "where", "note" } ]`; `note` may be `""` only for `compatible`.
- `check` (facing rows; `""` otherwise): golden capture — caller action to record against the baseline, what to capture, assertion against the target.

Rows predating the facing-API fields lack `exposure`, `compat`, `check`; treat them as not classified and the next run covering their area adds them.

## Register: findings and conclusions

`{ "updated", "pins": [ { "repo", "ref", "sha", "role" } ], "conclusions": [], "findings": [] }`

**`pins`**: the latest run's; `role` `baseline|upcoming|client|target|prior-art`.

**`conclusions`** (one per area, rewritten each run covering it): `area`; `verdict` `compatible|breaks clients|breaks consumers|risks only|not reviewed`; `text` two to five sentences (holds, breaks, unknown); `unspecified` client-used contracts the design leaves open (design-stage); `updated`.

**`findings`**:
- `id` `<area>-<n>`, never reused; `title` one line; `area`; `severity` `Critical|High|Medium|Low`; `status` `open|accepted|fixed|withdrawn`.
- `problem`: shortest concrete statement of what's wrong or missing (card summary, shown as **What's wrong/missing**). `difference`: one or two sentences, shown as **Reason** right after it. `impact`: who breaks, when, how.
- `tags` any of `inherited`, `upcoming`, `design-stage`, `consumer-unproven`, `security-lead`, `migration` (the migration branch serves the contract differently from the structure it moved, `main` before the move).
- `contracts` ids; `clients` (as in Contract catalog) and `consumers` (services, user tools) whose use is proved.
- `baseline` `<repo>@<sha> <path:line>: <what it does>`; `target` `<repo>@<sha> <path:line>` or `<doc> § <section>`, then what it does.
- `check` golden capture, required for Critical; `security_lead` one line or `""`.
- `decision` required when `accepted` (who, where written); `fixed_at` required when `fixed` (verified target pin); `reason` required when `withdrawn`; `first_seen`, `last_checked`.

## Presentation: the compact human view

An editorial companion, not a second source of verdicts. Severity, status, tags and `fixed_at` always come from Register.
- `version`: integer `1`.
- `findings`: object keyed by exactly the register's finding IDs (no missing or extra).
- `comparison`: 1–4 rows `{ "aspect", "baseline", "recorded" }`, one-line strings, the mismatch at its cited pins.
- `consequences`: 1–3 one-line bullets on the effect of that mismatch.
- `actions`: 1–3 one-line bullets summarizing the recorded decision or next work; never grant approval or change scope.
- `delivery`: one line on implemented behaviour and remaining acceptance; no copied status or pin fields.
- `flows`: 0–3 diagrams `{ "label", "steps": [2–6 one-line strings] }`, only where identity, timing or a handoff benefits.
- Every listed field is required; unknown fields are rejected; text must be nonempty.
- Author summaries from the cited evidence; never shorten source paragraphs by automatic sentence slicing.
- When delivery changes, review the summary with the register so an old mismatch is not shown as a current regression.
- Accepted does not mean fixed; deferred work stays unresolved; code delivery does not imply live-client or CI acceptance. Flow captions distinguish a recorded failure, required behaviour and verified code.

Retain the full explanation, history, decision, check, dates and verified revision in Notion. After presentation edits, save and read back both the summary and its source records. Security leads remain in the authorized private review workflow.

## Run history: one line per run

Append to the Notion run history: `- <date> | <target> | <scope> | <target pin> | <result: new findings, status changes, catalog changes>`
