# Decisions and reasons

Status: accepted for local implementation/rehearsal as of 2026-11-12. Production activation remains gated by [rollout evidence](rollout.md). These decisions apply to the included scope, not the entire service fleet.

## Authority and scope

| Decision | Reason and consequence |
| --- | --- |
| Migrate before activation. | Bulk data and catalogue identities must exist before traffic moves. Capture must bridge the snapshot to later commits; merely importing rows at different times is insufficient. |
| Production remains the write authority during coexistence. | One authority avoids concurrent same-entity conflicts and permits fallback while every accepted shared write still exists in production. Independent target writes remain denied. |
| Synchronization flows only production → dcl-one. | Current transformations merge or reshape records and are not generally reversible. Two-way writes need separate conflict, provenance and deletion rules; they are not implicitly enabled by adding a second database. |
| Keep clients unchanged for the initial sync rollout. | Row capture sees committed writes without new client headers or request duplication. This removes stable client operation IDs from the source-only critical path. |
| Retain the hybrid design, but expand its source-only lane first. | It can later exercise proven native target commands, while database sync covers changes that cannot safely be duplicated. Native command compatibility is not needed merely to copy production facts. |
| Begin with scoped Genesis Places favorites, then likes/dislikes. | They have relatively direct canonical keys and bounded effects. Both fact sets now feed the same derived counters. This is evidence for these mappings, not for all Places or all services. |
| Exclude shop and credits for security reasons. | Financial settlement, treasury, rewards and executable work need their own ownership/security review. Both marketplace imports target shop and therefore remain excluded, including marketplace favorites. Signatures/rentals remain deferred because of shop-derived dependencies. |
| Treat catalogue ownership as a prerequisite. | Interaction IDs must identify existing canonical, non-mirrored Genesis places. Adapter v3 captures scoped Genesis catalogue changes with interactions; conflicting provenance, worlds or native overlays stop application. Removal intents stay inert. See [catalogue decisions](places-catalogue.md). |

## Service architecture and capture

| Decision | Reason and consequence |
| --- | --- |
| Run a removable standalone service in the dcl-one monorepo. | Shared schemas, Rust dependencies and tests stay together, while deployment, stopping and retirement remain independent of the normal API. The worker is not linked into the serving backend. See the [ADR](../docs/adr/2026-11-12-removable-migration-sync-service.md). |
| Use the existing target schema migration system. | One bootstrap and additive migration path avoid a second schema owner. Worker metadata lives in `migration_sync`; mapped business facts remain in Explore. |
| Separate capture and apply processes/credentials. | A source read-only capture role can retain input even when target application is paused. Only explicit installation needs source DDL rights; the apply process needs target access. |
| Use an explicit source-local transactional outbox for the pilot. | Deployed logical CDC capability is unverified. Local triggers record changes in the same transaction and cover included writes that bypass HTTP. They never call dcl-one. This is not WAL decoding. |
| Do not use triggers that synchronously write the other DB. | Remote failures or slowness would affect production transactions, while schema mapping and conflict problems would remain. |
| Serialize captured source transactions with one local lock. | A committed order must not depend on sequence allocation before another transaction commits. The lock covers both favorite and like writes. Its contention cost is unmeasured and is a rollout gate. |
| Establish the snapshot under that same source lock. | A repeatable-read snapshot and capture boundary must agree. Release the lock before exporting the full data or contacting the target, so target availability never holds the source lock. |
| Bound the capture scope, baseline and transactions. | Explicit UUID allowlists and limits prevent silently broadening the pilot or truncating oversized input. Unsupported input stops rather than being partially accepted. |
| Preserve separate observed, durably captured and applied positions. | Receiving input is not evidence of target convergence. Source contact freshness is also required, including during idle periods. Outbox positions use hexadecimal cursors but are not real WAL LSNs. |
| Require a new baseline to expand a v1 favorites-only stream to v2 interactions. | A checkpoint recorded before likes were captured cannot prove complete like history. The additive target upgrade permits both adapter identities but never relabels old streams or changes their checkpoints. |
| Reject TRUNCATE and unsupported capture history. | Row-level deletion history cannot be reconstructed after an unrecorded truncate or gap. Trigger bypass, source replacement, DDL and exhausted history require stopping and a reviewed recovery/baseline procedure. |

## Apply and optional native replay

| Decision | Reason and consequence |
| --- | --- |
| Apply facts, versions, derived counts and checkpoints atomically. | A crash must not leave progress ahead of business state. Preserve source transaction boundaries, including transactions touching both interaction tables. |
| Keep favorite and like tombstones separate. | The same wallet/place pair can have both records; deleting one must not overwrite the other's history. Source order prevents delayed updates from resurrecting deleted state. |
| Recompute counts from accepted facts. | Copying source counter increments as well as executing mutations would double count. Preserve vote value, activity and both like timestamps; map source likes to target local facts. |
| Keep one ordered coordination lane per source pilot. | Native replay and transformed SQL must not overtake each other. No fictional global LSN is assumed across independent source databases. Cross-source dependencies need their own coordination. |
| Execute optional duplicate commands only after production acceptance. | Concurrent speculative execution could mutate dcl-one even when production rejects the command. Return production's response and replay asynchronously from committed evidence. |
| Record source command, mutation and response in one transaction. | A gateway timeout cannot reveal whether production committed. The durable receipt binds retry identity to the authenticated actor and semantic command, and returns the original response on retry. This is why the optional Places handler changes exist. |
| Resolve target execution through a database receipt, not HTTP success. | A response can be lost after commit, or success can be reported without the expected mutation. Native target changes and their receipt commit together; the coordinator verifies them before advancing. |
| Never routinely reapply rows already reproduced by native execution. | Applying both mechanisms would duplicate effects or overwrite newer state. Mixed dual-favorite transactions containing like changes are rejected, not partially suppressed. Ordinary source-only transactions may contain both. |
| Block inconsistent heads; retry transport uncertainty without skipping. | Unknown outcomes require retained identity and ordering. Divergent invariants must not be bypassed with blind SQL repair. Explicit blocked-head retry requires pause and the expected cursor; payloads and checkpoints stay unchanged. |
| Snapshot-covered commands do not execute again. | The baseline already contains their results. Generations and immutable boundaries prevent old queued work from replaying over a replacement baseline. |

## Runtime, effects and retirement

| Decision | Reason and consequence |
| --- | --- |
| Use a minimal follower listener rather than normal server startup. | API flags, missing credentials and separate database leases do not disable every job/provider. The dedicated follower starts no normal jobs, consumers, mirrors or external providers. |
| Keep production as sole job/effect owner. | Copying rows does not prevent duplicate email, reward, publication, room cleanup or queue acknowledgements. Copy business results only; do not automatically activate copied executable work, leases or pending delivery records. |
| Keep social mutation sockets, voice and provider work on production. | A database copy cannot reproduce live sessions or notification delivery. Duplicate voice accepts can interact with the same room even when conversation uniqueness rules exist. Social expansion requires separate protocol and effect handling. |
| Gate target reads on scope, generation, freshness and convergence. | A ready pilot must not present uncaptured IDs as synchronized. Current reads are private verification endpoints, not public Places API replacements. Readiness is sampled; it is not cross-database linearizability. |
| Reconcile canonical facts and derived counts at stable boundaries. | Equal row counts alone can hide wrong keys, votes or grants. Reject changing source/target boundaries or mismatched adapter/scope metadata instead of calling an inconsistent comparison successful. |
| Retain history; do not prune automatically. | Receipts and tombstones support retries, conflict detection and outage recovery. Retention limits and safe removal must be measured and agreed. |
| Fence and drain before transferring authority. | Stop old intake/writers/jobs, reach a final boundary, reconcile, then enable the new owner. Startup advisory guards do not fence old binaries, manual writers or a runtime that loses its guard connection. Deployment controls remain necessary. |
| Do not promise simple rollback after independent target writes. | Production may no longer contain accepted target changes or effects. Recovery then needs a separately tested preservation process. |

See [repository rationale](repository-changes.md) for which of these choices require code or database changes outside dcl-one.

## Multiple-source expansion

See [coordination decisions](multiple-sources.md), [Events schedules](events-schedules.md) and [social settings](social-settings.md). Each source retains its own baseline, immutable scope and cursor because independent databases have no shared commit order. All new mappings are source-only. Reconciliation holds repeatable-read snapshots across size preflight and row decoding to prevent concurrent changes bypassing allocation limits. Oversized transactions block without advancing capture; they are never split or skipped.
