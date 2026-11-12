# Coverage and service constraints

This is the boundary of the current implementation and the reasons broader coverage remains gated. Static schema and writer review does not establish which versions or job configurations are deployed.

## Implemented interaction mapping

| Source fact | Target | Treatment and reason |
| --- | --- | --- |
| Places `user_favorites` | `explore.user_favorites` | Canonical wallet/Genesis UUID; capture insert/update/delete and preserve activity/creation time. Optional native favorite replay needs a committed command receipt. |
| Places `user_likes` | `explore.user_likes` | Preserve true/false vote, activity, creation/update timestamps; map provenance to local target facts. Source-only execution keeps clients unchanged. |
| Favorite/like counts | `explore.place_interaction_counts` | Derive favorites, likes and dislikes from accepted facts; never apply source increments a second time. |
| Source transaction order | `migration_sync` inbox, generations and separate favorite/like versions | Atomically account for all changes. Independent tombstones prevent one relation's deletion from replacing another's history. |

See the [machine-readable scope](../services/migration-sync/pilot-contract.json), [capture](../services/migration-sync/src/capture.rs) and [apply implementation](../services/migration-sync/src/store.rs). Both tables share one snapshot and transaction stream. Installed source scope and persisted target scope must match. Catalogue metadata now shares that boundary through adapter v3. World interactions remain excluded. See [Places catalogue](places-catalogue.md).

The source-local serialization lock also affects writes outside the selected row scope on the three captured Places tables. That cost must be measured. Capture covers any normal SQL writer of the scoped facts, but no actual production scheduled-job dataset has yet been validated. An orphan cleanup in dcl-one's mirror is not evidence of an equivalent production job.

## Why mappings are per dataset and writer

A request, internal call, job and operator script may modify the same table. A table name alone cannot prove that a second request reproduced a particular change. Source-only capture copies authoritative results; optional dual execution requires exact operation provenance. Derived state, business facts and executable work require different treatment.

| Area | Feasibility and reason for deferral |
| --- | --- |
| Places catalogue and Events schedules | Scoped catalogue and schedule mappings are implemented. Future IDs, worlds, event references and active cleanup remain excluded; see [catalogue](places-catalogue.md) and [schedules](events-schedules.md). |
| Events and attendance | Deletion can mean cancellation rather than row removal; recurrence and mirror adoption can overwrite ownership. Community events also depend on social authorization and notification production. |
| Social settings and notification read state | Three settings fields are implemented for scoped wallets; see [settings](social-settings.md). Provider privacy enforcement and notification read/delivery state remain excluded. |
| Friendships and communities | Ordered pair/history merges, roles, bans and multi-table invariants make simple reverse mappings unsafe. Ranking is another independent writer. |
| Worlds permissions/settings/storage | JSON-to-relational conversion, namespace collapse and merged keys require provenance. Membership revocation needs enforcement, not just a copied deletion. Environment values and assets require additional key/storage handling. |
| Badges and referrals | Evaluator state, merged progress and reward ownership must agree. Some legacy referral state is in Redis and is not captured by PostgreSQL row replication. |
| Builder and camera | Publication, curation, blobs, generated object IDs, moderation and ranking span rows and external storage. Assess restricted metadata subsets before whole-service synchronization. |
| Voice/comms/sessions | LiveKit rooms, sessions, leases, provider receipts and cleanup state are not ordinary replicated facts. Keep one active effect owner and a deliberate handover. |
| Signatures/rentals | Some records are structurally close, but signed facts must remain intact and target status depends on excluded shop-derived state. Deferred. |
| Marketplace favorites, shop, treasury, credits | Excluded for security reasons. Marketplace favorites is a database-backed dataset, but its target is in shop; its name does not make it part of the Places pilot. |

The [forward batch mappings](../tooling/migration/sources) are a starting point, not proof of reversible or continuous synchronization. [PR #195](https://github.com/decentraland/dcl-one/pull/195) addressed reviewed importer omissions and guards; historical reports about missing referral, broadcast, event or badge fields must be checked against the chosen release rather than repeated as current bugs.

## Cross-service and scheduled-work constraints

- **Voice:** a social operation can call the gatekeeper internally. Duplicating both can affect the same room; “one conversation per user” does not make retries harmless. Keep production ownership until a separate protocol/effect test proves safety. See the [reviewed voice acceptance path](https://github.com/decentraland/social-service-ea/blob/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec/src/logic/voice/voice.ts#L101).
- **Social protocols:** moving social mutations to HTTP and using notification-only sockets is a possible expansion approach, not completed client work. Existing mutation sockets stay on production. Notifications require an intentional delivery path; database synchronization does not emit WebSocket events. Target [HTTP](../services/catalyst/social/src/rpc/http.rs) and [stream](../services/catalyst/social/src/rpc/stream.rs) contracts need a scoped compatibility review.
- **Permissions:** copying a membership deletion does not automatically invoke a [world access callback](../services/catalyst/worlds/src/core/mod.rs). Delayed revocation can leave access open. Do not apply a generic public-counter freshness allowance to authorization.
- **Events:** [event notification intents](../services/catalyst/explore/src/events/notifications.rs) are executable work. Synchronizing facts and enabling their processor are separate decisions.
- **Schedulers:** [fleet leases](../services/catalyst/jobs/src/lib.rs) coordinate jobs in one database, not two independent deployments. API write flags do not disable every raw task or consumer. Missing provider credentials can still allow data to be consumed or marked skipped. Use the minimal follower and explicit deployment ownership.
- **Queues and provider calls:** retain committed outcomes and ambiguous operations. A source message acknowledged before the intended database write cannot be recovered merely by CDC. Lease loss or shutdown does not prove that an in-flight provider effect was rolled back.
- **Storage and mirrors:** only one ingestion owner may write an overlapping projection. A replica with incomplete references must not delete shared production blobs. Live media and object bytes need separate handling.

## Findings versus gates

Concrete protocol and mapping behaviors justify restrictions above. Their production frequency, deployed routes, role grants, cloud schedules, retained history and provider guarantees remain unverified. The current implementation has local fault-test evidence for its scoped interactions, catalogue, schedules and settings; it does not demonstrate complete service parity, independent target authorization, real job coverage or safe financial migration.
