# Social settings synchronization

## Decision and scope

Synchronize committed rows from the source `social_settings` table into target `social.user_settings` for an explicit immutable canonical-wallet allowlist. This is source-only fact synchronization; it does not enable target social APIs, provider effects or privacy enforcement. Existing clients and source HTTP/RPC handlers remain unchanged.

Source review baseline: [social-service-ea revision 143e9aa3](https://github.com/decentraland/social-service-ea/tree/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec). The [initial schema](https://github.com/decentraland/social-service-ea/blob/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec/src/migrations/1741805019806_social-settings.ts) and [reaction visibility migration](https://github.com/decentraland/social-service-ea/blob/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec/src/migrations/1772795904215_add-situation-reactions-visibility.ts) define the source fields. Deployed revision and privileges remain unverified.

| Source field | Target treatment | Reason |
| --- | --- | --- |
| `address` | Same canonical lowercase wallet key | Current source writers normalize addresses. Historical case variants are rejected for this scoped adapter rather than silently merging identities and mishandling later deletions. |
| `private_messages_privacy` | Preserve `only_friends` or `all` exactly | These are security-relevant facts; invalid values stop mapping. |
| `blocked_users_messages_visibility` | Preserve `show_messages` or `do_not_show_messages` exactly | Do not infer a default for a missing or malformed field. |
| `show_situation_reactions` | Preserve `show` or `hide` exactly | All source row fields must be present in captured upserts. |
| No source timestamp | Target `updated_at` is local observation metadata | Source has no update timestamp. Identical fact replay does not manufacture a new target timestamp. Reconciliation excludes this target-only field. |
| Missing/deleted row | Delete target row | Absent-row reader privacy defaults to `all`, while a newly inserted DB row defaults to `only_friends`. Fabricating a default row would change behavior. |

The [existing batch mapping](../tooling/migration/sources/social-service-ea/user_settings.load.sql) is the forward-schema reference. The [continuous mapper](../services/migration-sync/src/social_settings.rs) preserves absence and validates explicit values. Capture records full resulting rows and deletes, including address changes, in source transaction order.

## Why a source application PR is not required

The source [database adapter](https://github.com/decentraland/social-service-ea/blob/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec/src/adapters/friends-db.ts) performs settings upsert/delete in ordinary SQL. The [local capture installation](../services/migration-sync/source/social-settings.sql) observes those commits, including writers that bypass RPC. It needs an operator-reviewed source database installation and explicit grants, not synchronization calls in each application writer or new client operation IDs.

All capture functions run as the invoking source role. Installation and grants must be coordinated before writes resume. Capture's local ordering lock adds contention to source settings writes, including out-of-scope ones; measure it before rollout. Runtime capture is read-only and never calls dcl-one from a source transaction. TRUNCATE is rejected while capture is installed. Trigger bypass or unsupported DDL invalidates the capture contract.

No source application PR is currently planned for this mapping. If native dual settings execution is proposed later, an additional acceptance/effect protocol must be designed first; this mapping does not make it safe automatically.

## Why privacy effects are not replayed

The source [settings RPC](https://github.com/decentraland/social-service-ea/blob/143e9aa3c2fe21c9e44e9541e7c8dc2561a9d1ec/src/controllers/handlers/rpc/upsert-social-settings.ts) also changes Gatekeeper metadata. Tightening privacy reaches Gatekeeper before the row write; loosening writes the row before contacting Gatekeeper. A failed provider operation can therefore leave committed facts and provider metadata at different boundaries. The sync worker copies committed facts only and does not repeat these calls or interpret a row as proof of provider convergence.

Production remains the privacy/effect authority. The target must not serve independent privacy admission decisions or live messaging from these eventually synchronized facts. Membership, friendships, blocks, mutes, notifications, voice and providers are outside this mapping. A separate coupled-workflow proof is required before activating them.

## Acceptance and remaining gates

Tests cover exact settings values, source defaults versus absent rows, no-op replay, multiple writes in one transaction, rollback, deletion, TRUNCATE rejection, out-of-scope mutations and rejection of noncanonical source keys. The ordered coordinator must additionally prove snapshot continuity, generation/scope checks, blocked-head ordering and durable checkpoint behavior for this source.

Deployment needs the actual source writer/role inventory, representative case-collision checks, contention/retention measurements, source restart/restore rehearsal and a deliberate privacy-read boundary. Table capture is not proof of a real scheduled job or of cross-service effect synchronization.
