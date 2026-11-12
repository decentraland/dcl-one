# Staging rehearsal: scope and acceptance

The next deployment is an isolated synchronization rehearsal with unchanged clients. Production remains the only owner of business writes, jobs and external effects. No users move to dcl-one during this rehearsal. Run mutations and fault scenarios against disposable staging source clones, never live production sources. Shop, treasury and credits are excluded for security reasons.

## What runs

Use one target database and three independent source streams: Places catalogue/interactions, selected Events schedules, and selected Social settings wallets. Each source has its own capture process and source read credentials; each stream has an apply process. The [worker runbook](../services/migration-sync/README.md) provides the install, bootstrap, capture and apply commands. The [installation package](staging-installation.md) covers source grants and checks.

Do not run normal Catalyst serving, mirrors, consumers or scheduled jobs against this target. Schedule/settings equality is not evidence that target event notifications or privacy enforcement are correct. Do not share production provider credentials with the rehearsal.

## Before installation

Record the following with the rehearsal results, excluding credentials and user payloads:

| Input | Required decision or evidence |
| --- | --- |
| Releases | Source, target, schema and adapter revisions; deployed versions must match the reviewed schema |
| Scope | Explicit Genesis UUIDs, schedule UUIDs and canonical wallet allowlists; include existing and absent rows |
| Writers | Every application, job, maintenance and migration role that can modify captured tables |
| Isolation | How all source writers are fenced during installation/grants, and all competing target writers stay off |
| Budgets | Maximum source latency increase, capture/apply lag, storage growth, outage recovery time and retained history |
| Duration | Intended coexistence period and representative peak/job windows to cover |
| Ownership | Operator, alert recipient and stop/recovery decision owner |

No numerical production budget has been established. Set these before a measured rehearsal; example alert thresholds are not acceptance criteria. Source trigger installation affects writes outside the selected scope too, so a small allowlist does not bound source lock contention.

## Rehearsal sequence

1. Install the target schema in a dedicated database. Install each source adapter and explicit grants while source writers are fenced; run the source checks before resuming writers. Preserve capture history.
2. Bootstrap each stream independently while ordinary source writes continue. Do not import overlapping facts separately and then label them a consistent snapshot. Bootstrap starts application paused.
3. Start capture for all three streams. Confirm durable inbox growth while application remains paused. Resume application and measure catch-up; one paused or failed stream must not advance another's checkpoint.
4. Exercise scoped inserts, updates, deletes and rollback. Include a mixed Places catalogue/interaction transaction, a schedule update with unchanged `updated_at`, settings row absence, and non-UUID world interactions outside the Places scope.
5. Run the actual source POI job against a controlled dependency fixture and confirm captured category changes. Existing SQL-fixture tests do not establish end-to-end job coverage. Events recurrence/notifications and Social ranking remain outside the current mappings; do not count them as covered.
6. Stop/restart capture and apply independently, interrupt source connectivity, repeat input, and create a deliberately unsupported scoped row in a disposable scenario. Verify no skipped transaction, partial target commit or false healthy status. A source correction creates a later transaction; it does not repair retained invalid input. Retry an immutable blocked head only when a mapper/dependency fix makes that exact payload applicable. A capture-time rejection may have no inbox head yet. Otherwise stop and follow a reviewed new-baseline/history procedure; never edit or skip retained input.
7. At stable source boundaries, reconcile each stream. Record source generation, source/captured/applied positions and equality results. A moving boundary is inconclusive; a mismatch or failed command is a failure, never an accepted match.
8. Run sustained capture/application through the agreed workload and outage windows. Measure source write latency and lock waits, retained outbox/inbox bytes, backlog, lag and recovery time. Test restore/failover with new generation/baseline rules separately; a source restored under the same generation cannot be assumed safe.

## Read-only sampling

The [soak script](../services/migration-sync/scripts/soak.sh) accepts an optional fourth source argument. Existing Places invocations remain compatible:

```sh
services/migration-sync/scripts/soak.sh places 1440 60 places
services/migration-sync/scripts/soak.sh events 1440 60 events
services/migration-sync/scripts/soak.sh settings 1440 60 social-settings
```

Run these as separate supervised processes, each with its corresponding `MIGRATION_SYNC_SOURCE_DATABASE_URL` and the common `MIGRATION_SYNC_DATABASE_URL` supplied securely through the environment. Stream names above are examples and must match bootstrap. The script reads status, metrics and health, then reconciles; it does not start capture or apply. It stops at the first failure and accepts 1–1,000,000 samples with intervals of 1–86,400 seconds. Because reconciliation rejects a changing boundary, use this script for stable-window checks and collect health/metrics separately during continuous traffic. Sampling success does not measure throughput or prove effect safety.

## Exit decision

Proceed to public-read compatibility work only when every included stream reconciles, measured budgets hold, source jobs behave within those budgets and restart/outage recovery loses no committed facts. Attach the measured results and unresolved failures to the decision record.

Stop target use on unknown history, schema drift, unreviewed writers, loss of source identity, blocked transactions or exceeded budgets. Keep production serving; do not fail writes over to dcl-one. Capture may continue while application is paused if history and capacity remain safe. Do not drop history or disable triggers as an automatic error response.

Full service migration still requires the missing mappings in the [Places audit](places-coverage-audit.md) and [Events/Social audit](events-social-coverage-audit.md), plus explicit provider/job ownership handover. This rehearsal does not authorize that cutover.
