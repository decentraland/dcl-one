# Hybrid migration: decision summary

**Migrate before activation. Reproduce compatible operations in both backends; build production → dcl-one DB sync only for changes that cannot be reproduced reliably by those operations.** Classify each operation and writer, not an entire service or table. This replaces the earlier “expand source-only sync first” plan.

## Decisions

1. **Production remains authoritative.** It accepts or rejects each operation and owns the user response. The target executes accepted compatible operations with durable retries, stable identity and ordering. Two simultaneous HTTP calls alone are not a reliable replication protocol.
2. **Do not copy the same mutation twice.** A native target receipt proves duplicate execution. Residual DB sync must identify uncovered changes and share ordering with replay; it must not overwrite or reapply changes already reproduced natively.
3. **Favorites and likes use native command replay.** Their API operations do not justify continuous row copying by themselves. Both now use bounded native replay, including dislike and withdrawal. Unreceipted interaction mutations stop replay instead of silently falling back to SQL copying.
4. **Build residual sync only after evidence.** Job, queue, webhook or internal-call outcomes are candidates when the original operation cannot be replayed safely with the same identity and result. Bypassing the gateway alone is not proof: first assess whether that operation can join the replay protocol.
5. **Keep one owner of external effects and live sessions.** Production retains jobs, provider calls, voice, notifications and mutation WebSockets until a separate handover is proven. Copying rows does not deliver notifications or enforce revocations.
6. **Shop, treasury, credits and marketplace datasets are excluded for security reasons.** Dependencies on them do not authorize expanding this migration.
7. **No public activation yet.** The isolated follower exposes private migration endpoints only. Public API compatibility, signed paths, complete data scope, readiness enforcement and session/effect ownership remain unresolved.

## Kept, removed and still needed

| Status | Scope and reason |
| --- | --- |
| Native execution | Favorite and like/dislike/withdrawal setters use durable source acceptance, ordered native target execution and atomic receipts. The gateway returns production's response. |
| Selective sync | Only the source POI job's effective POI membership bit is mapped. There is no equivalent existing request to duplicate. Other categories/catalogue data are not copied. Unknown writers stop the shared stream. |
| Not included | Broad interaction/catalogue/schedule/settings row replication and generic gateway read activation. Public target APIs and the remaining operation contracts are still incomplete. |
| Next | Rehearse the integrated command/POI paths, then classify the next API command or residual writer. No whole-service sync is implied by this pilot. |

Caller-supplied stable IDs remain the default. For these idempotent setters only, opt-in gateway `request-attempt` IDs allow unchanged clients: each actual production acceptance is replayed in order. Separate client retries are not deduplicated as one logical intent. Source/target receipts remain required. Initial migration may copy baseline rows; that is different from continuously copying compatible request outcomes.

## Review requests

[Hybrid worker #706](https://github.com/decentraland/dcl-one/pull/706) (depends on [initial migration #195](https://github.com/decentraland/dcl-one/pull/195)), [Places #901](https://github.com/decentraland/places/pull/901), [Events #1032](https://github.com/decentraland/events/pull/1032), and [gateway !446](https://dcl.tools/ops/cloudflare-workers/-/merge_requests/446). All are published; none is deployed.

## Evidence and follow-up

- [Repository-wide coverage and priorities](repository-service-inventory.md): in-scope Worlds/content/storage priorities, explicit scope exclusions, retained services and missing source checkouts. Includes the complete 246-checkout screening catalog.

- [Places command protocol](places-hybrid-commands.md) and [POI residual justification](places-poi-residual.md): exact implemented boundaries and source-change reasons.
- [Notifications gateway contract](notifications-gateway-contract.md): read-only paths, mutating GET endpoints, native command candidates and queue reliability blockers.
- [Next Events command contract](events-command-contract.md): schedule edits first, creation/RSVP blockers, and the local recurrence completion fix. Events replay is not enabled.
- [Operation classification and backlog](operation-classification.md): duplicate candidates, residual candidates and acceptance requirements.
- [Gateway feasibility](gateway-feasibility.md): routing, signed requests, WebSockets and public-read blockers.
- Service evidence: [Places](places-coverage-audit.md), [Events/Social](events-social-coverage-audit.md), [Worlds/creator/content](worlds-creator-sync-audit.md), [remaining services and uncatalogued dependencies](remaining-services-sync-audit.md).
- [Repository/PR rationale](repository-changes.md): changes retained or removed and why another repository would need a PR.
- [Alternatives](alternatives.md) and [retained pilot runbook](../services/migration-sync/README.md).

No production installation or deployment is authorized by these documents. Source revisions are pinned in the audits; deployed roles, routes, queues, jobs, load and recovery budgets still need verification. Gateway routing cannot replace that inventory.
