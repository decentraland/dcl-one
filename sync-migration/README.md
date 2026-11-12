# Sync migration decisions

Updated 2026-11-12. This directory records the selected migration strategy, its reasons, alternatives, implementation boundaries and required follow-up. It is the maintained decision record; passing local tests is not production activation approval.

**Current choice:** migrate before activation, keep production authoritative, and expand one-way production → dcl-one database synchronization with unchanged clients. The Places pilot covers favorites, likes and dislikes. Native duplicate execution is optional and currently limited to favorite commands. Shop and credits remain excluded for security reasons.

| Document | Contents |
| --- | --- |
| [Decisions and reasons](decisions.md) | Authority, architecture, capture, ordering, recovery, jobs and retirement |
| [Alternatives](alternatives.md) | Four rollout proposals, snapshot PoC, bidirectional sync and trigger tradeoffs |
| [Repository changes and PR rationale](repository-changes.md) | Why dcl-one, Places or the gateway need changes; required versus optional work |
| [Coverage and service constraints](coverage.md) | Implemented mappings, exclusions, schema differences and cross-service risks |
| [Implementation and validation](implementation.md) | Local code, test evidence, commits and known limits |
| [Rollout and backlog](rollout.md) | Remaining work, acceptance criteria, prolonged operation and cutover |

The [service runbook](../services/migration-sync/README.md) owns executable commands and operational instructions. The [architecture ADR](../docs/adr/2026-11-12-removable-migration-sync-service.md) owns the standalone service boundary. Keep both aligned with these documents.

For each future decision, record the affected scope, choice, reason, alternatives rejected or deferred, required repository changes, validation and unresolved gates. Update the repository rationale before opening a PR into another service. Do not equate table capture coverage with verified scheduled-job coverage or infer production behavior from local fixtures.
