# Places coverage audit

Reviewed 2026-11-12 against Places `ba6674f6f13c2715be3e480d4b01b6979cde79fc` and the local dcl-one sync implementation. This is a source-code inventory, not confirmation of deployed schema, task configuration, external scripts or traffic.

## Decision

Follow the [decision summary](README.md): favorites and likes are duplicate-command candidates. Catalogue job/consumer outputs are residual candidates only after deterministic input replay is assessed. The broad source-only adapters were removed; this inventory justifies further classification, not installation. Production retains consumers, schedulers and provider effects. The retained favorite receipt protocol requires a source handler change and stable retry identity.

## Tables and writers

| Dataset | Writers / evidence | Current result and remaining work |
| --- | --- | --- |
| `places` | [Deployment application](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/CheckScenes/task/applyDeploymentDecision.ts) | Catalogue capture is not implemented in the retained pilot. Effective categories, visibility, ranking and raw positions need operation-level classification. New UUIDs outside the immutable allowlist are omitted. Worlds are rejected. Redeployments that create a replacement UUID can disable the old scoped row while leaving the new row outside scope. |
| `user_favorites`, `user_likes` | [Interaction updates](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/shared/entityInteractions.ts) | Favorite baseline/native command replay retained; continuous row-copy paths removed. Likes require native compatibility work. World-name IDs excluded. HTTP calls, internal SQL and normal operator writes hit the same triggers. |
| `place_categories`, `categories` | [Category application](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/PlaceCategories/tasks/poi.ts) | Effective `places.categories` is a residual-output candidate. Category registry activity and normalized associations are not mirrored; category-management and history parity remain incomplete. |
| `place_positions` | [Position ownership updates](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/PlacePosition/model.ts) | Source maintains a normalized parcel-to-base map. Target reads raw catalogue positions instead; source rows are not copied. Prove overlapping replacements, parcel removals and read-route equivalence. |
| `content_ratings` | [Deployment and moderation history model](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/PlaceContentRating/model.ts) | Classify rating/admin commands before choosing replay or residual mapping; moderator, message and historical decisions are additional dependencies for history/admin reads. |
| `worlds` and world catalogue rows | [World model](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/World/model.ts) | Excluded. Need world identity, settings/privacy, interactions and tombstone dependencies as one reviewed expansion. |
| `world_undeployments`, `world_scene_undeployments`, `world_deployment_position_watermarks` | [World replacement application](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/CheckScenes/task/applyDeploymentDecision.ts) | Excluded ingestion-order safeguards. Do not start a second world ingestion consumer with only catalogue facts. |
| `check_scenes_logs` | [Scene application log writes](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/CheckScenes/task/applyDeploymentDecision.ts) | Not copied. Source diagnostic history is different from target ingestion ownership and cannot safely activate target work. |
| `road_positions` | [Road registry migration](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/migrations/1786665600000_create-road-positions.ts) | Not captured. Record a static reference version and compare required read behavior before rollout; migrations can change it. |
| `users` | [User model](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/User/model.ts) | Model and historical migration exist; no live caller was found in reviewed source. Verify deployed scripts before classifying it as unused. |
| Task framework storage | [Task setup](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/server.ts) | Not copied. Production keeps task ownership; copying job receipts/leases would not establish cross-database exclusivity. |

Historical `place_activities`, `place_activity_daily` and `deployment_tracks` are dropped by source migrations; do not present them as active sync omissions without checking the deployed schema. See [activity retirement](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/migrations/1666718810532_deprecate-place-activity-and-place-activity-daily.ts) and [deployment-track retirement](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/migrations/1666734959705_deprecate-check-deployment.ts).

## Scheduled and external writers

| Writer | Cadence in source | Sync behavior |
| --- | --- | --- |
| [POI category task](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/PlaceCategories/tasks/poi.ts) | Daily | Writes normalized categories and then effective `places.categories`. Potential residual scope is effective outcomes, not the external POI feed or intermediate association history. No capture adapter is approved yet. Source code uses multiple operations; a job failure can leave intermediate committed source state, which sync must faithfully preserve rather than inventing a final result. |
| [Scene consumer](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/CheckScenes/task/checkScenes.ts) | Every 10 seconds when configured | Consumes SQS via the dispatcher. Scoped catalogue changes require classification; queue acknowledgement, source processing logs, new IDs and world ingestion safeguards are not reproduced. Keep target queue consumer off. |
| [Hot-scenes task](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/RealmProvider/tasks/hotScenesUpdate.ts) | Minutely | Refreshes in-memory provider cache. No DB outbox can synchronize this memory; live population read parity needs a separate read/provider design. |
| [World-live-data task](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/World/tasks/worldsLiveData.ts) | Minutely | Refreshes in-memory world occupancy. Excluded with worlds; not a captured database write. |
| [Ranking replacement endpoint](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/Place/model.ts) | Caller-owned schedule; not established by this repository | Replaces automated rankings and clears omitted entries in SQL. Scoped catalogue results are residual candidates. Discover the external job configuration and rehearse a representative batch before choosing a replay or residual contract; establish transaction-size bounds without dropping outcomes. |

The registered tasks are visible in [server setup](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/server.ts). A separate legacy hot-scenes interval also exists in [the module](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/modules/hotScenes.ts); its presence alone does not establish that a production bundle loads it. Deployed task roles, cloud cron jobs, operator scripts, source triggers and queue configuration still need an environment inventory.

## Concrete compatibility defect found and corrected

The adapter initially compared its UUID scope keys directly with source character IDs. Places explicitly documents `places.id` as `character(36)` in [its model](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/Place/model.ts#L39); the original migrations use the Gatsby `Type.UUID` shorthand, which resolves to `CHAR(36)`. Interaction columns use the same initial shorthand; the [entity rename migration](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/migrations/1770729788000_rename-place-id-to-entity-id.ts) renames them without changing their type. World-name interactions are therefore possible without PostgreSQL UUID-typed columns.

Direct UUID/character equality can fail during installation, bootstrap, catalogue writes or reconciliation. UUID-only test fixtures hid this incompatibility. The fix compares textual representations in source queries/triggers, keeps scope validation canonical, and never casts arbitrary source world IDs to UUID. The optional favorite command path receives the same correction. Target schema and wire IDs are unchanged.

The retained [capture regression](../services/migration-sync/tests/capture.rs) uses character IDs and unscoped world-name facts. The source identifier fix remains in baseline/reconciliation and native favorite commands. The catalogue adapter was removed with the unjustified broad sync expansion. Source schema changes still require compatibility review; no claim about deployed column variants is made.

## Prioritized next work and PR rationale

1. Prove favorite native replay through the source handler and gateway, including client retries. Classify likes/dislikes before adding their replay support.
2. Assess POI, ranking and deployment inputs for deterministic replay. Only then choose exact residual catalogue outcomes to capture; include replacement identities and deletes.
3. Prove discovery/parcel/category response compatibility and live-cache behavior before public reads. Finite UUID scopes do not cover future deployments or general listing queries.
4. Assess worlds, normalized category history and moderation only for explicit rollout requirements.

The retained Places handler change records authenticated source acceptance and response atomically; it is needed for reliable native duplicate execution. A source-local capture installation may later record approved residual outcomes, but no table-wide sync PR is justified merely because a job bypasses the gateway. See [operation classification](operation-classification.md) and [repository rationale](repository-changes.md).
