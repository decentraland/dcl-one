# ADR: Migration decisions under sync-migration

- Status: Accepted
- Date: 2026-11-12

## Decision

Maintain one concise decision summary at [sync-migration/README.md](../../sync-migration/README.md), linking only to supporting operation classification, repository rationale and pinned service audits. Remove superseded deployment plans rather than leaving conflicting entrypoints. These are explicitly requested tracked
project documents, not private task scratch files.

Keep the [standalone service ADR](2026-11-12-removable-migration-sync-service.md)
and [executable runbook](../../services/migration-sync/README.md) as the architecture
and operational sources. Link them from the decision documents rather than copying
command instructions. Record new decisions and the reasons for external service
changes here before proposing their PRs. Separate implemented/tested scope from
production readiness and deferred alternatives.
