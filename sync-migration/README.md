# Hybrid migration: decision summary

**Migrate before activation. Reproduce compatible operations in both backends; build production → dcl-one DB sync only for changes that cannot be reproduced reliably by those operations.** Classify each operation and writer, not an entire service or table. This replaces the earlier “expand source-only sync first” plan.

## Decisions

1. **Production remains authoritative.** It accepts or rejects each operation and owns the user response. The target executes accepted compatible operations with durable retries, stable identity and ordering. Two simultaneous HTTP calls alone are not a reliable replication protocol.
2. **Do not copy the same mutation twice.** A native target receipt proves duplicate execution. Residual DB sync must identify uncovered changes and share ordering with replay; it must not overwrite or reapply changes already reproduced natively.
3. **Favorites and likes are duplicate-request candidates.** Their API operations do not justify continuous row copying by themselves. Favorites have a bounded native replay implementation; likes still need compatibility/retry tests. Unreceipted favorite mutations stop replay instead of silently falling back to SQL copying.
4. **Build residual sync only after evidence.** Job, queue, webhook or internal-call outcomes are candidates when the original operation cannot be replayed safely with the same identity and result. Bypassing the gateway alone is not proof: first assess whether that operation can join the replay protocol.
5. **Keep one owner of external effects and live sessions.** Production retains jobs, provider calls, voice, notifications and mutation WebSockets until a separate handover is proven. Copying rows does not deliver notifications or enforce revocations.
6. **Shop, treasury, credits and marketplace datasets are excluded for security reasons.** Dependencies on them do not authorize expanding this migration.
7. **No public activation yet.** The isolated follower exposes private migration endpoints only. Public API compatibility, signed paths, complete data scope, readiness enforcement and session/effect ownership remain unresolved.

## Kept, removed and still needed

| Status | Scope and reason |
| --- | --- |
| Keep | Pre-activation baseline, durable source favorite receipts, ordered native replay, target receipts, isolated follower, gateway production routing/shadow comparison. These support reliable duplicate execution and its verification. |
| Remove | Default row-copy adapters for scoped likes, catalogue, Events schedules and Social settings; their extra metadata/commands/tests and multi-source installation package. They preceded operation-level justification. Research is retained. |
| Remove | Newly added gateway read-activation mode. It did not establish command compatibility or solve the missing public follower APIs. |
| Next | Prove favorite replay through the actual source handler/gateway; classify likes and other API mutations; select the first demonstrated residual writer and implement only that mapping. |

The retained command pilot needs stable operation IDs and source/target receipts. “No client changes” is a preference, not a reason to default to DB sync: prove retry identity with existing clients or explicitly resolve that constraint before activation. Initial migration may copy baseline rows; that is different from continuously copying compatible request outcomes.

## Evidence and follow-up

- [Operation classification and backlog](operation-classification.md): duplicate candidates, residual candidates and acceptance requirements.
- [Gateway feasibility](gateway-feasibility.md): routing, signed requests, WebSockets and public-read blockers.
- Service evidence: [Places](places-coverage-audit.md), [Events/Social](events-social-coverage-audit.md), [Worlds/creator/content](worlds-creator-sync-audit.md), [remaining services and uncatalogued dependencies](remaining-services-sync-audit.md).
- [Repository/PR rationale](repository-changes.md): changes retained or removed and why another repository would need a PR.
- [Alternatives](alternatives.md) and [retained pilot runbook](../services/migration-sync/README.md).

No production installation or deployment is authorized by these documents. Source revisions are pinned in the audits; deployed roles, routes, queues, jobs, load and recovery budgets still need verification. Gateway routing cannot replace that inventory.
