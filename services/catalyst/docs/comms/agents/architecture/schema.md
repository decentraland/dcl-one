# Architecture records: schema

These fields describe architecture reports and proposed record updates. The main session reconciles them per [the reporting workflow](../../README.md). Working reports stay in `.agent-work/`. Validate the field and status requirements before returning the report.

**General rules**
- Dates are `YYYY-MM-DD`. Text fields may carry `` `code` `` spans.
- An empty value is `""` or `[]`; never leave a field out.
- `area`: `system`, `pulse`, `connector`, `gatekeeper`, `voice`, `streaming`, `presence`, `scene-host`, `rooms` or `bus` (divergence areas plus `system`).
- Ids never change once used; rows are never deleted, only retired by status.

## Architecture model: what the system is

`{ "updated", "components": [], "edges": [], "channels": [], "abstractions": [] }`

**`components`** (crates, services, binaries, modules, external systems):
- `id` kebab-case (`catalyst-comms`, `livekit`); `kind` `crate|service|binary|module|external`; `parent` containing id or `""`; `layer` number or `""`; `area`.
- `owns` one line per responsibility; `offers`, `needs` channel and abstraction ids; `state` what it writes as single writer.
- `earns_place`: required for `crate`, `service`, `binary`.
- `status` `proposed|accepted|built|retired`; `where` `<doc> § <section>` or `<repo>@<sha> <path>`.

**`edges`** (keyed by `from`, `to`, `kind`):
- `from`, `to` component ids; `kind` `depends` (crate dep), `calls` (in-process via an abstraction), `channel` (between processes).
- `status` optional `active|retired` (omitted = `active`). Retired edges stay visible, keep endpoints and verdict, are excluded from active counts and cycle/layer checks; record retirement pin and reason in `note`.
- `channel` required when `kind` is `channel`; `abstraction` id or `""`; `note` one line.
- `verdict` `allowed|violation|proposed|not reviewed`. A `depends` edge to the same or a higher layer must be `violation`.

**`channels`**:
- `id` `<area>.<name>`; `status` optional `active|retired` (omitted = `active`); retired channels stay visible, are excluded from active counts, record retirement pin and reason in `schema` or `failure`, and their connection edges are retired explicitly.
- `area`; `producers`, `consumers` component ids; `transport` `in-process|nats|http|websocket|webtransport|udp|livekit|webhook|database|process|other`; `encoding`; `schema` source of truth (`<repo>:<path>` or `<doc> § <section>`).
- `delivery` `at-most-once|at-least-once|exactly-once-effect`; `ordering` scope; `idempotency` key, required for `at-least-once`.
- `versioning`; `limits` (size caps both sides, rate limits, slow-consumer policy); `failure` (timeouts, retries, fail-open/closed, effect when the peer is down); `auth`; `external` divergence contract ids.
- `verdict` `sound|gaps|unsound|proposed|not reviewed`; `gaps` one line each, required for `gaps` or `unsound`.

**`abstractions`** (traits, module layers, wrappers, generics, flags):
- `id` `<component>.<name>`; `kind` `trait|module|wrapper|generic|flag`; `component`; `purpose`.
- `implementations` `[ { "name", "use": "production"|"test", "where": "<path:line>" } ]`.
- `earns_place_by` `two-implementations|test-seam|boundary|external-requirement|none`.
- `verdict` `earns its place|merge|inline|remove|not reviewed`; `earns its place` needs `earns_place_by` other than `none`. `changes` change request ids.

## Register: judgements

`{ "updated", "pins": [], "conclusions": [], "decisions": [], "technologies": [], "changes": [] }`

**`pins`** (latest run): `{ "repo", "ref", "sha", "role" }`, `role` `target|design|prior-art|baseline`.

**`conclusions`** (one per area, rewritten on every run covering it): `area`; `verdict` `sound|sound with changes|needs redesign|not reviewed`; `text` two to five sentences (what holds, what must change, what is unknown); `metrics` counts at the pin (e.g. `{ "crates": 1, "services": 3, "channels": 9 }`) or `{}`; `updated`.

**`decisions`**. Decisions from changes: when an accepted or implemented change (or coherent group) establishes or revises an architectural choice (ownership, boundaries, deployment, technology, protocol, interface), add or update the decision card covering it; link it to the change-request IDs and link those back through their `decision` field; record context, options, choice, consequences and `current_stack`. Mark it `accepted` only when the user's verdict approves the choice (implementation is evidence, not approval); otherwise keep it `proposed`. Include the proposed decision update in the report for the same task.
- `id` `decision-<n>`; `title`; `area`; `status` `proposed|accepted|rejected|superseded`; `date` proposed on.
- `status_changed_at`: when the current status took effect — `YYYY-MM-DD` if only the date is evidenced, else ISO 8601 with timezone. Set on creation, update on every transition, preserve on text edits and re-checks. Unknown for older entries: `""` (shown **Not recorded**); never substitute proposal date, last-check date or build time.
- `context` two or three sentences; `options` one line each; `choice` and why the others lost; `consequences` fixes, costs, constraints.
- `current_stack`: required for every decision dated 2026-10-19 or later, backfilled on older ones. `{ "summary", "rows": [ { "aspect", "baseline", "as_built", "chosen" } ] }`, one row per aspect bearing on the choice: `baseline` production at its pin (`repo@sha path:line`), `as_built` dcl-one at the target pin, `chosen` the option. `summary`: parity, gap closed or divergence, and why. Without it the decision is not ready for a verdict.
- `revisit_when` observable reopening event; `decided_by` required when `accepted|rejected|superseded` (who, when, where written); `superseded_by` required when `superseded`; `sources` docs and `path:line`.

**`technologies`**: `id` kebab-case; `name`, `purpose`; `area`; `need` (measured); `criteria` `[ { "criterion", "rating", "evidence" } ]` with criterion `fit|operations|maintenance|licence|security|platform|consistency|exit` and rating `good|acceptable|poor|unknown`; `verdict` `adopt|adopt with conditions|trial|hold|reject`; `conditions` required for `adopt with conditions`; `experiment` required for `trial` (what, how, pass mark); `alternatives`; `decision` id or `""`; `checked` date.

**`changes`** (change requests):
- `id` `<area>-change-<n>`; `title`; `area`; `kind` `technology|interconnection|channel|abstraction|complexity`; `priority` `blocking|should|could`; `status` `open|accepted|done|declined|withdrawn`.
- `status_changed_at` as for decisions; for an initial `open`, `first_seen` is the creation date; for completion, the date it was recorded as verified, not the commit date.
- `decision` related decision ID(s), whose sources link back.
- `rule` as named in the agent's *Rules*; `where` `<repo>@<sha> <path:line>` or `<doc> § <section>`; `problem` one or two sentences; `change` concrete edit; `gain` countable (lines, types, hops, tasks, edges, channels, dependencies removed); `work_package` or `""`; `leads` for `divergence-analyst` or `comms-security-reviewer`.
- `done_at` required when `done` (verified target pin). `reason` required when `accepted|declined|withdrawn`: who, when, where, and the current-stack comparison shown (baseline, as built, proposal); declining a `blocking` request also names the user's decision.
- `first_seen`, `last_checked` dates.

## Run history: one line per run

Append to the run history: `- <date> | <mode> | <target> | <target pin> | <result: decisions, change requests, model and status changes>`
