# Rollout, backlog and operational gates

Current priority: expand source-only coverage with unchanged clients, then run a representative staging rehearsal. Keep the optional native favorite protocol disabled unless deliberately testing it. No item below grants production deployment approval.

## Active work packages

| ID | Deliverable and rationale | Current state | Acceptance before activation |
| --- | --- | --- | --- |
| H1 | Freeze mapping scope and deployed writer inventory. Unknown scripts, queues or schemas can invalidate completeness. | Places interaction contract and immutable scope implemented. | Pin source/target releases; verify every scoped writer, role, trigger, alias and dependency; agree numerical operating budgets. |
| H2 | Isolate target writes, jobs, mirrors and effects. API flags and separate DB leases do not establish global ownership. | Dedicated follower and startup guards implemented. | Fence older binaries and direct writers; test realistic pending work stays inert; validate any public read adapter. |
| H3 | Align capture and baseline. Timestamp batches can miss late commits/deletes. | Local outbox and snapshot-aligned Places v3, Events schedules and Social settings bootstraps implemented. | Verify deployed grants, source capacity, failover and history recovery; rehearse writes throughout export. |
| H4 | Optional durable native command execution. Timeouts cannot prove a transaction failed. | Favorite source/target receipts and gateway protocol implemented locally. | Only for native replay: publish/review coordinated PRs, add stable client IDs, verify signed routes and deployed ingress. Not a blocker for source-only sync. |
| H5 | Selective, atomic transformation and ordered progress. Copying native mutations again causes divergence. | Favorites and likes/dislikes implemented with tombstones and counts. | Each new mapping proves identities, updates, deletes, mixed transactions, derived state and effect behavior. Unknown mappings block. |
| H6 | Representative fault rehearsal. Unit success does not establish coexistence behavior. | Local PostgreSQL/HTTP pilot suites pass. | Test production-like volume, actual source job writes, outages and recovery; reconcile canonical state and any intended effects. |
| H7 | Operate for the intended duration. Lag, storage and retained history accumulate. | Health/metrics, alerts, recovery, reconciliation and soak script implemented. | Run extended soak; wire alerts; set measured budgets; test restore/failover and retention boundaries; assign operators. |
| H8 | Expand dependency-complete workflows. Independent tables can omit permissions or notifications. | Scoped Places catalogue, Events schedules and social settings implemented and locally tested. | Review each contract before claiming coverage; separately prove a real job-owned dataset and a coupled permission/notification workflow. |

The next mappings are candidates, not blanket permission to synchronize an entire service. Financial exclusions remain unchanged. Each service workstream must extend [repository rationale](repository-changes.md) before an external PR is proposed.

## Staging order

1. Pin revisions and approve the explicit nonfinancial scope. Inventory source writers, target dependencies, queues, providers and blobs. Confirm the source role/grant plan before installing triggers.
2. Prepare a dedicated target with normal APIs/jobs/mirrors off. Install the normal schema. Places bootstrap imports scoped catalogue and interaction facts together; any existing catalogue must have reviewed Places provenance. Leave scoped interaction, schedule and social-settings baselines empty for their consistent bootstraps.
3. Install reviewed source capture and grants in a controlled change. Bootstrap both interaction fact sets at the same boundary. Existing Places v1/v2 streams need a reviewed new v3 baseline rather than relabelling old progress.
4. Start capture and apply separately, with the minimum roles. For unchanged-client source-only mode, no client ID, source HTTP feature flag or native replay URL/token is required.
5. Compare canonical facts and derived counts, then test inserts, updates, deletes, mixed transactions, worker restarts, duplicate delivery, poison input, stale contact and history gaps. Test an actual job-owned dataset before claiming scheduled-job coverage.
6. Measure source lock contention, end-to-end lag, queue/storage growth and recovery speed. Exercise alerts and stable-boundary reconciliation through the intended outage window and a sustained soak.
7. Add a read cohort only after public response/authentication compatibility and read-after-write behavior are proven. The private follower endpoint is not sufficient by itself.

If optional native replay is included, additionally test production rejection, a production commit with lost response, missing/false target success, delayed target execution, ID conflicts and a later delete. Validate that exactly one execution mechanism accounts for each production change.

## Weeks or months of coexistence

Version source schema, capture adapter, mapping and target release together. Keep receipts, input history, tombstones and identity maps for the agreed retry/outage horizon; the current worker does not prune them automatically. Test rolling upgrades, restore and failover before relying on them.

Set separate freshness policies for public counters, read-after-write and authorization. Disable unhealthy scopes rather than serving unknown permissions. Alert on stale source contact, blocked heads, unapplied age, lag, storage growth and missing metrics. Example alert thresholds in the runbook are rehearsal defaults, not agreed production SLOs.

Production owns all business jobs, provider actions, queue intake and live media. Do not enable a target worker because its tables look equal. Copied pending work must remain inert unless a deliberate handover preserves accepted work, completion receipts and ambiguous outcomes. Shared production blobs need a single deletion policy.

## Recovery and final cutover

During one-way coexistence, return affected traffic to production and pause target use when validation fails. Do not fail shared writes over to dcl-one during a production outage. Retry an immutable blocked head only after correcting its cause; never skip the cursor or silently replace evidence.

At final ownership transfer:

1. Fence old command intake and all scoped source writers/jobs; drain or durably retain in-flight work and provider ambiguity.
2. Capture through the final source boundaries, drain target application and reconcile facts, dependencies, counters and relevant effects.
3. Stop follower/replay intake and incoming synchronization before enabling independent target writes and the selected jobs.
4. Archive the retained history for the agreed horizon. Remove triggers, slots or worker metadata only through an explicit operator procedure after the transition is proven.

Before independent target writes, the old owner can resume if the cutover is aborted. After target writes or external effects, rollback requires a tested way to preserve them; restoring an earlier snapshot alone is insufficient.

## Open operational decisions

Deployed role names, actual PostgreSQL/CDC capabilities, cloud cron/queue configuration, intended cohort/duration, numerical interruption/freshness/outage/retention budgets, operator assignments, production provider guarantees and reverse recovery after cutover are still unverified or unset. Keep these explicit; do not replace them with assumptions drawn from static code review.

## Rehearsal package

Use the [source installation package](staging-installation.md) and [staging acceptance procedure](staging-rehearsal.md). The [Places](places-coverage-audit.md) and [Events/Social](events-social-coverage-audit.md) audits distinguish existing capture from missing service workflows. No live staging results are claimed by these instructions.
