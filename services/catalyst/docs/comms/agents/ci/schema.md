# Ci records: schema

These fields describe the Notion ci records and proposed updates. The main session saves and verifies them per [the record workflow](../../README.md). They are not repository files. Validate the field and status requirements before saving.

**General rules**
- Dates are `YYYY-MM-DD`. Text fields may carry `` `code` `` spans.
- An empty value is `""` or `[]`; never leave a field out.
- Ids never change once used; rows are never deleted, only retired by status.
- `tier`: `advisory` (every PR, never blocking; new checks start here until they have clean runs), `pr-required` (every PR, blocking), `merge-queue` (again on the merge candidate), `nightly` (default branch, scheduled), `release` (release candidate).

## Check catalog: the check catalog

`{ "updated", "checks": [] }`, each check:
- `id` `<category>.<name>` (e.g. `architecture.layer-direction`); `category` `build|test|architecture|supply-chain|contract|database|performance|semver|coverage|security|release|pipeline`; `summary` one line.
- `tool` with pinned version (e.g. `cargo-deny 0.18.x`); `tier` (for `planned`, the target tier: it still lands in `advisory` first, except in the bootstrap run); `budget_seconds` number.
- `source` `{ "role", "ref" }`, role `comms-architect|divergence-analyst|comms-security-reviewer|decision|org-policy|comms-ci-engineer`; `scope` crates/paths or `whole workspace`.
- `status` `planned|active|quarantined|retired`; `where` workflow/config `path:line` at the pin, `""` while `planned`.
- `last_result` `{ "result": "pass|fail|flaky|not run", "pin", "date", "run" }`.
- `allowlist` `[ { "item", "reason", "expires" } ]`; `quarantine` required when `quarantined`: `{ "reason", "since", "expires", "owner" }`; `notes` one line each.

## Register: gates, health and requests

`{ "updated", "pins": [], "budgets": [], "toolchain": [], "gates": [], "flaky": [], "requests": [] }`
- **`pins`** (latest run): `{ "repo", "ref", "sha", "role" }`, role `target|default-branch|prior-art`.
- **`budgets`** (one per tier): `tier`, `limit_minutes`, `p50_minutes`/`p90_minutes` over the last 20 runs (numbers, `""` before runs), `measured`.
- **`toolchain`** (Rust toolchain, MSRV, every CI tool and Action): `tool`, `pinned`, `latest` stable when checked, `checked`, `action` (`""` when current, else the update and why or why not).
- **`gates`** (one per gated PR revision): `pr` `<owner/repo>#<n>`, `pin` head sha, `date`, `verdict` `pass|fail|infrastructure` (CI failed, no verdict on code), `failures` `[ { "check", "class": "regression|flaky|infrastructure|check-bug", "evidence", "action" } ]`, `missing` check ids that should have run.
- **`flaky`**: `id` `flaky-<n>`, `test` `<crate>::<path>::<test>`, `check`, `rate` (e.g. `3/50`), `status` `watching|quarantined|fixed|removed`, `quarantined_until` required when `quarantined`, `owner`, `cause` or `""`, `first_seen`, `last_seen`.
- **`requests`** (work for others): `id` `ci-request-<n>`, `title`, `kind` `flaky|slow-test|nondeterminism|missing-test|toolchain|supply-chain|release|pipeline`, `priority` `blocking|should|could`, `status` `open|accepted|done|declined|withdrawn`, `for` `implementer|main session|the user`, `where` `<repo>@<sha> <path:line>` or a CI run, `change`, `gain` countable (minutes saved, flakes removed, advisories closed), `work_package` or `""`, `done_at` required when `done`, `reason` required when `declined|withdrawn`, `first_seen`, `last_checked`.

## Run history: one line per run

Append to the Notion run history: `- <date> | <mode> | <target> | <target pin> | <result: checks added or changed, gate verdict, flakes, requests>`
