# ADR: Migration decisions under sync-migration

- Status: Accepted
- Date: 2026-11-12

## Decision

Maintain migration strategy, alternatives, reasons, repository/PR rationale,
coverage and rollout gates in the root [sync-migration](../../sync-migration/README.md)
directory, as requested for this project. These are explicitly requested tracked
project documents, not private task scratch files.

Keep the [standalone service ADR](2026-11-12-removable-migration-sync-service.md)
and [executable runbook](../../services/migration-sync/README.md) as the architecture
and operational sources. Link them from the decision documents rather than copying
command instructions. Record new decisions and the reasons for external service
changes here before proposing their PRs. Separate implemented/tested scope from
production readiness and deferred alternatives.
