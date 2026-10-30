# Comms agent workflow and records

Read [comms-crate.md](comms-crate.md) for ownership and
[implementation-pipeline.md](implementation-pipeline.md) for delivery and review.
For Pulse porting work, read [pulse-rust-migration.md](pulse-rust-migration.md).

## Notion is the ledger

Architecture models, decisions and change requests; compatibility contracts and
divergence findings; CI checks, gate verdicts and run history; lineage; readiness
and measurement evidence belong in the team's Notion workspace. This public
repository keeps reusable instructions and [ADRs](../../../../docs/adr/).
Ledger data, private design notes, run logs, captures, rendered pages and export
builders are not repository inputs. A public ADR preserves an accepted choice
and rationale without copying private discussion or review findings.

Before work that depends on a ledger verdict, the main session resolves the
existing record through the connected Notion workspace or the user's supplied
context, verifies the target, and passes the scoped records to the specialist.
Use existing IDs; do not create a second ledger or infer acceptance from code.
If access or the intended destination is missing, request that context and keep
the dependent verdict or update pending while continuing independent work.

## Update and verify

1. Read the current Notion records and pin each implementation and baseline
   revision before changing a claim. Preserve unrelated edits and record IDs.
2. Specialists return proposed updates and evidence in their report or root
   `.agent-work/`, using the relevant schema below. They do not accept their own
   proposals. Security findings go only to the private review workflow.
3. The main session applies the reviewed updates to the existing Notion records
   in the same task, recording the user's verdict, provenance and transition
   dates where required. Update related contracts, conclusions and lineage.
4. Read the saved content back and compare changed IDs, statuses, evidence and
   verdicts with the intended updates. Report completion only after verification;
   if it fails, report the pending IDs and reason. A local report is not a saved
   ledger update.

## Record schemas

- [Architecture](agents/architecture/schema.md): model, decisions, technologies,
  change requests and run history.
- [Divergence](agents/divergence/schema.md): contracts, findings, conclusions and
  their presentation.
- [CI](agents/ci/schema.md): checks, gate verdicts, budgets, flakes and requests.
- [Crate review](../crates/agents/review-method.md): ownership and distribution.

These schemas describe record fields, not files to generate or commit. Notion
page identifiers, private URLs, credentials and workstation paths belong in
authorized session context, not in these instructions. Public examples use
placeholders. Verify each source reference at its cited revision; old review
evidence does not establish current compatibility, deployment or test success.
