# Operation classification and implementation backlog

The [decision summary](README.md) is authoritative. This is a research matrix, not permission to install a table-wide capture adapter. A single table can contain outcomes from both replayable requests and residual writers.

## Classification requirements

Before implementing a lane, record the operation, every writer, source and target identities, transaction boundary, deletes, dependent rows, generated values and external effects.

**Duplicate execution is eligible** when authenticated production acceptance is durable; target behavior and IDs are compatible; retries cannot repeat effects; order is preserved; and committed native receipts establish completion after lost responses. The gateway may deliver or identify commands, but cannot infer DB commit from an HTTP status alone.

**Residual sync is eligible** only when the documented operation cannot reliably reproduce the required target facts and no selected compatible replay path covers it. Define exact changed facts and transformations. Prove the classifier distinguishes them from replayed changes, including mixed transactions; source/target receipts and progress must not advance ahead of accepted mutations. A job or internal call may be replayable, so its transport alone does not decide this classification.

## Current candidates

| Operation/writer | Proposed lane | Evidence still needed |
| --- | --- | --- |
| Places favorite set/unset | Duplicate execution; bounded receipt protocol retained | Real handler/gateway integration, existing-client retry identity, concurrent set/unset ordering and loss/retry scenarios |
| Places like/dislike/delete | Duplicate candidate; no continuous row adapter retained | Native behavior, stable identity, activity/timestamps, derived counts and retry tests |
| Places POI categories, ranking replacement, scene-consumer catalogue outcomes | Residual candidates | Determine whether source job/deployment inputs can be replayed deterministically; otherwise capture exact effective outputs, replacement IDs and deletes, not all catalogue writes by default |
| Events schedule edits and RSVP/cancel | Duplicate candidates | Shared generated IDs, source's separate counter commits, authorization and target response compatibility |
| Events recurrence job | Residual candidate | Replaying the same occurrence computation versus copying source outcome; repair unawaited source write before using job completion as evidence |
| Social settings, mutes, friendship/community commands | Duplicate candidates, not proven | WebSocket command adaptation or durable source command capture, pair/history/role invariants; suppress repeated provider and notification effects |
| Social ranking and queue-generated metrics | Residual candidates | Same event identity, deduplication, source-owned computation and cross-service ordering |
| Notification read receipts, opt-outs and subscriptions | Duplicate candidates | Broadcast/notification identity, retry semantics and no duplicate delivery |
| Queue-created notification/broadcast facts | Residual candidates | Durable producer acceptance, IDs, expiry/deletes; fix acknowledgement on failed persistence first |
| Badge catalog/admin commands | Duplicate candidates | Version IDs, permission checks and publication assets |
| Badge evaluator and presence progress | Residual candidates | Event/progress provenance, resets and deletes; one reward/effect owner |
| Worlds permission/storage edits and deployments | Assess replay before sync | Identical namespace/scene identity, authorization, upload bytes and safe revocation behavior; multi-step uploads are not one duplicate JSON request |
| Worlds owner/quota/eviction jobs | Residual candidates | Exact retained outputs, permission/scene/blob dependencies; timestamp-based imports miss some owner updates |
| Camera and Builder mutations | Assess replay before sync | Generated IDs, complete object/version transfer, publication/moderation effects and delete behavior |
| Content/profile entities | Reuse existing content replication where compatible | Isolated peer ingestion, accepted pointers/deletes and verified blobs; not another generic DB adapter |
| Gatekeeper voice/streaming, provider webhooks and cleanup | Single effect owner / explicit handover | No duplicate LiveKit calls; durable outcomes and stale-permission policy before any fact projection |
| Auth, Linker and feature/config distribution | Explicitly retained external dependencies | Scope decision before replacement; onboarding DB, Redis/socket state, signing secrets and KV are not implicitly migrated |
| Shop/credits/treasury/marketplace | Excluded | Separate security and service-handover scope |

Detailed source links and writer evidence are in the four service audits linked from the summary. “Candidate” means implementation is not authorized merely by appearing in this table.

## Ordered next work

1. Prove the retained favorite native replay end to end; reject unreceipted source mutations rather than silently copying their rows.
2. Classify and implement like/dislike native execution if compatible. Resolve the unchanged-client identity constraint explicitly; no invented guarantee that repeated HTTP calls are exactly once.
3. Select one residual writer after comparing deterministic input replay with outcome mapping. Add only its source capture, provenance classifier, transformation and tests.
4. Test replay and residual updates touching the same entity, mixed transactions, deletes, delayed retries, source rejection and unknown target commit outcomes. No second application of native results.
5. Build a public read surface that runs without target jobs/providers and validates every dependency's scope and freshness. The current private follower is not this surface.
6. Rehearse against disposable source clones, then verify actual deployed routing/writers and measure contention, lag, retention and recovery before a public cohort.

The removed default source-only implementations are not an available fallback deployment. Adding one back requires operation-level justification under this contract. Initial bulk migration and durable command receipts remain necessary and are not continuous DB mirroring.
