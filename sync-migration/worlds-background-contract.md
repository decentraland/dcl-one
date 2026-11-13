# Worlds: multiplayer storage and background outcomes

**Decision:** player values written by authoritative multiplayer scenes are an uncovered mutation path. Cover their accepted storage operations without executing the scene twice. For Worlds content, use production-committed owner/quota facts; do not rerun external-data decisions at the target. Physical eviction of already-retired scenes does not justify an active-scene DB mirror.

Reviewed 2026-11-12 against refreshed defaults: [SDK multiplayer e0d2e7ef](https://github.com/decentraland/sdk-multiplayer-server/tree/e0d2e7ef688b2db41293e4b4bed87226dba74071), [World Storage de0178e1](https://github.com/decentraland/world-storage-service/tree/de0178e1d78fa50188824a83bf28e02b8fe427af), and [Worlds content ebbf8d41](https://github.com/decentraland/worlds-content-server/tree/ebbf8d41d07973e317892067c765107d3512938e). These source pins support the coverage assessment. The bounded implementation and local validation are described below; production traffic has not been captured.

## Player values: a real coverage gap

The multiplayer parent [mints scene-scoped storage delegations](https://github.com/decentraland/sdk-multiplayer-server/blob/e0d2e7ef688b2db41293e4b4bed87226dba74071/src/adapters/bevy-engine-process.ts#L1128). The [delegation contract](https://github.com/decentraland/sdk-multiplayer-server/blob/e0d2e7ef688b2db41293e4b4bed87226dba74071/src/logic/storage-gatekeeper/types.ts) identifies signed requests to `storage.decentraland.*`. The user confirms this traffic bypasses the gateway; code establishes a storage HTTP boundary, not a direct multiplayer SQL write. Production DNS/ingress routing has not been inspected.

World Storage accepts `PUT /players/:player_address/values/:key`, per-key DELETE, per-player clear and scene-wide player clear. Its [handler](https://github.com/decentraland/world-storage-service/blob/de0178e1d78fa50188824a83bf28e02b8fe427af/src/controllers/handlers/player-storage/upsert-player-storage.ts) passes the accepted value and resolved world/place/player/key to [quota-locked transactional storage operations](https://github.com/decentraland/world-storage-service/blob/de0178e1d78fa50188824a83bf28e02b8fe427af/src/logic/storage-operations/component.ts). The [adapter](https://github.com/decentraland/world-storage-service/blob/de0178e1d78fa50188824a83bf28e02b8fe427af/src/adapters/player-storage/component.ts) persists JSON values and physically deletes rows.

**Preferred coverage:** capture an accepted storage command in the same source transaction, then replay it through a private target native operation. This covers multiplayer and other callers even when they bypass the edge gateway. Alternatively, route the existing storage hostname through the gateway, preserving signed URLs and delegation verification; that still needs durable source acceptance. Neither option needs client changes or duplicate scene execution. The source-acceptance approach is implemented for the bounded player pilot; real ingress and production load still require rehearsal.

**If storage ingress and source acceptance remain unchanged:** selective committed-change capture for player values is needed to keep the follower current. Include inserts, updates, per-key deletes and bulk clears; timestamp polling cannot recover physical deletes. Capture only changes without native receipts, and stop on ambiguous provenance. Do not apply both row outcomes and replay to the same mutation.

Required contract for either lane:

- Bind the accepted source `(world_name, place_id, player_address, key)` and scene/deployment identity to the target namespace. Do not re-resolve a delayed command against a newer deployment.
- Order clears and writes together, including redeployment and namespace changes; define an atomic baseline/capture boundary and deletion tombstones.
- Preserve production acceptance despite later delegation expiry. The private replay protocol authenticates durable source evidence, not an expired user request; it must not become a public authorization bypass.
- Validate limits, JSON/value-size behavior and namespace collisions against the [baseline mapping](../tooling/migration/sources/world-storage-service/MAPPING.md). Recompute winners when many source keys map to one target key; avoid timestamp-based last-write guessing.
- Invalidate target caches through native operations or an explicit apply hook. Keep production scene execution authoritative; concurrent target gameplay writes are incompatible with a one-way authoritative follower unless separately scoped.

The same caller path may write world values or read env values; caller identity alone must not trigger replication of every storage table. Collaborators remain derived from scene metadata, not a source-table copy.

## Owner changes and quota blocks: committed outcomes are required

The [12-hour owner job](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/adapters/update-owner-job.ts) resolves owners of deployed `.dcl.eth` worlds, writes changed owners, evaluates wallet quota and deletes stale blocks while preserving wallets it could not evaluate. Ownership, quota allowance and whitelist inputs can change between source and target runs. There is no existing gateway request that reproduces the exact job decision.

**Recommended residual projection:** committed owner changes keyed by normalized world name, and `blocked` insert/update/delete outcomes keyed by wallet. Map these to `worlds.worlds.owner` and `worlds.quota_blocks`, never platform-wide bans. A new internal command containing the resolved outcome would be another delivery mechanism for these facts, not proof that the original external calculation is replayable.

Capture requirements:

- Preserve block `created_at` and `updated_at`: the original creation time starts the 48-hour grace period. A repeated upsert must not restart it; a delete followed by a new block is a new episode.
- Capture **all block writers**, not just the cron. [Deployment post-commit hooks](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/adapters/entity-deployer.ts) and [undeployment logic](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/logic/worlds/component.ts) call `unblockIfUnderQuota`. Duplicating a deployment does not guarantee the same external quota result. Native deployment replay must suppress independent quota decisions if residual capture owns these facts.
- Use committed transaction evidence or a transactional outbox, including delete tombstones. The job has multiple commits; do not invent an atomic whole-job result. Owner updates still omit `updated_at`, so `--since` imports can miss transfers.
- Keep SNS publication on production. The [blocking component](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/adapters/blocking/component.ts) writes DB state and publishes separately. Copying a block proves neither delivery nor a live kick. Re-publication from the follower would risk duplicate notifications.
- Disable competing target [owner reconciliation](../services/catalyst/worlds/src/core/owners.rs) and [quota decisions](../services/catalyst/worlds/src/core/quota.rs), including request-triggered decisions. Validate cache/admission behavior and a freshness gate before public activation; stale ownership or block data is authorization-sensitive.

## Eviction: retention cleanup, not another undeployment

The [daily source eviction job](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/adapters/eviction-job.ts) defaults to seven days. Its [database operation](https://github.com/decentraland/worlds-content-server/blob/ebbf8d41d07973e317892067c765107d3512938e/src/adapters/worlds-manager.ts#L1634) only deletes `world_scenes` rows already marked `UNDEPLOYED` with `updated_at` before the cutoff. It does not delete active scenes or player values, and this path does not publish an undeployment event.

The target [evicts retired scene records](../services/catalyst/worlds/src/core/store.rs) separately. Therefore **do not add a general scene-deletion sync solely for this job**. First ensure the earlier undeployment command was replayed and the active scene/place identity retired. The existing baseline already omits/removes undeployed active scenes; source physical cleanup adds no new active-scene fact in that mapping.

Retired-history equivalence remains a separate retention decision. If required, apply an exact source retirement/eviction fact only to the matching retired identity, never to a replacement deployment. Otherwise retain target history until replay/blob-copy progress makes cleanup safe. Gate target GC against outstanding references; a new TTL calculation on the follower is not evidence the source evicted the same records. Keep pending upload cleanup source-owned.

## Remaining activation work

1. Rehearse direct multiplayer-origin writes and recovery with real ingress and credentials. The fixed scene binding deliberately stops on redeployment; supporting a moving deployment requires a separate ordered identity contract.
2. Measure source contention and run a multi-day recovery/retention soak. Before any public target traffic, provide target job suppression, cache/admission freshness gates and operational fencing.
3. Prove undeployment coverage before adding eviction handling. Test retirement followed by redeployment and delayed source eviction; the new active scene and player values must survive. Exact retired-history replication remains optional pending the retention contract.

## Implemented boundary

- Player commands: set, per-key delete, per-player clear and scene-wide clear. Source command capture includes no-op clears and actual acceptance order; unreceipted writes block the stream. No player-row copy fallback is enabled.
- Scope: explicit fixed world/place/scene bindings, matching preimported baseline and local target scenes. A changed binding stops replay. Genesis many-to-one mapping and continuing through redeployments are not supported by this slice.
- Worlds facts: scoped owner changes and block insert/update/delete, with original timestamps and target before-state verification. Transfers outside the approved wallet scope stop the stream instead of silently losing quota coverage.
- Effects: production owns scene execution, jobs, providers and notifications. Both new stream registrations fence normal target startup. Source application flags, SQL installation and credentials are explicit; nothing is automatically enabled on deployment.
- Native target operations: replay trusts source acceptance and uses the storage mutation shared with normal handlers. It does not re-run mutable quota/auth decisions; normal public handlers retain them. No target serving cache runs in the isolated rehearsal.

The installation and runtime commands live in the [worker runbook](../services/migration-sync/README.md). Staging traffic, contention, retention, recovery and multi-day soak remain unverified.

## Local validation (2026-11-13)

The combined worker passed 29 unit/CLI tests and all five real PostgreSQL integration suites (capture, player, POI, PostgreSQL replay and Worlds), Clippy with warnings denied, four schema checks and Rust formatting. Native player storage passed 62 tests and the runtime fence passed two tests. Fresh-install and upgrade storage functions match; the source player-capture SQL matches the worker copy byte-for-byte.

The [World Storage source change](https://github.com/decentraland/world-storage-service/pull/130) passed 516 unit tests, 16 PostgreSQL tests, TypeScript checks and ESLint. The [Worlds source change](https://github.com/decentraland/worlds-content-server/pull/561) passed nine tests, including real PostgreSQL lock contention, TypeScript checks and focused ESLint.

Regression coverage includes restricted source roles, rollback and ordered capture, transient database retry, original quota timestamps, delete/reblock, unknown writers, scope/identity changes, deeply nested JSON and escaped values at the storage size limit. Source writers acquire the capture lock before native row/quota locks to avoid reversed lock ordering. Transient database failures leave progress retryable; identity or state conflicts stop application.

These are local results, not production load, failover or deployment evidence. Source flags remain disabled by default and target application is restricted to an isolated rehearsal.
