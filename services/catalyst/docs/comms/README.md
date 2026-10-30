# Comms agent workflow and reports

Read [comms-crate.md](comms-crate.md) for ownership and
[implementation-pipeline.md](implementation-pipeline.md) for delivery and review.
For Pulse porting work, read [pulse-rust-migration.md](pulse-rust-migration.md).

## Public documentation and working reports

This public repository keeps reusable instructions and [ADRs](../../../../docs/adr/).
Ledger data, private design notes, run logs, captures, rendered pages and export
builders are not repository inputs. A public ADR preserves an accepted choice
and rationale without copying private discussion or review findings.

The main session supplies specialists with the task scope, target revision and
relevant prior decisions and review evidence. Re-verify earlier claims at their
cited revisions. Use existing record IDs when provided, and identify missing
context explicitly; implementation alone never establishes acceptance.

## Reporting workflow

1. Pin each implementation and baseline revision before making a claim. Preserve
   unrelated work and distinguish evidence from proposals.
2. Specialists return findings, proposed record updates and evidence in their
   report or root `.agent-work/`, using the relevant schema below. They do not
   accept their own proposals. Security findings stay in the private review workflow.
3. The main session reconciles reports, verifies changed IDs, statuses, evidence
   and transition dates, and records the user's verdict where required. Update
   public ownership guidance and ADRs when an accepted architectural choice changes.
4. Report verified outcomes, unresolved decisions and checks not run. Keep working
   reports in root `.agent-work/`; commit them only when the user explicitly asks.

## Report schemas

- [Architecture](agents/architecture/schema.md): model, decisions, technologies,
  change requests and run history.
- [Divergence](agents/divergence/schema.md): contracts, findings, conclusions and
  their presentation.
- [CI](agents/ci/schema.md): checks, gate verdicts, budgets, flakes and requests.
- [Crate review](../crates/agents/review-method.md): ownership and distribution.

These schemas describe report fields, not files to generate or commit. Private
workspace identifiers, URLs, credentials and workstation paths stay in authorized
session context. Public examples use placeholders. Old review evidence does not
establish current compatibility, deployment or test success.
