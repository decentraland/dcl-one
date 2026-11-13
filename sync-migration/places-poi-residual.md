# Places POI membership: selective residual contract

Status: implemented locally for a bounded pilot; not deployed or approved for general catalogue continuity. Follow the [decision summary](README.md): duplicate compatible operations first, capture only the outcome that no compatible gateway request can reproduce. This is not permission to restore broad catalogue replication.

## Why this outcome is different from favorite/like commands

The [daily POI task](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/PlaceCategories/tasks/poi.ts) has no inbound mutation request. It fetches the external POI list, resolves enabled source places at those positions, compares them with normalized POI memberships, and updates both `place_categories` and effective `places.categories`. These operations depend on the source's current catalogue and category associations.

Calling the external feed twice is not replay of the same accepted command. The [feed client](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/modules/pois.ts) caches for one hour, and the feed and catalogue can change between runs. The target [POI admin routes](../services/catalyst/explore/src/places/admin.rs) update `explore_api.lists_local`; they do not implement this effective source-category update. Two gateway requests to those routes cannot reproduce the source job result.

A future durable job-input protocol could make deterministic native replay possible, but it would also need the exact feed input and catalogue/category snapshot, a source commit boundary and equivalent target derivation. Those do not currently exist. A narrowly scoped committed-output mapping is therefore justified for **POI membership only**, subject to the provenance, baseline and ordering requirements below.

## Exact scope

The residual payload carries a canonical Genesis place UUID and one boolean indicating whether the source job's committed effective `places.categories` contains `poi`. It must identify the producer as `places_poi_category_update_v1`.

The target changes only the `poi` member in the existing row's extracted `categories` and `raw.categories`, preserving every unrelated category and raw field. It must not replace the whole category array from the source or copy deployment metadata, ranking, visibility, interactions or world rows. Initial catalogue/POI membership still comes from the consistent pre-activation migration.

The observation trigger records only selected places whose effective POI bit changes. It does not copy their category arrays. Untagged changes block the shared stream; they are never assumed to be job results. The [deployment consumer](https://github.com/decentraland/places/blob/ba6674f6f13c2715be3e480d4b01b6979cde79fc/src/entities/CheckScenes/task/applyDeploymentDecision.ts) also updates categories and preserves POI/featured membership. Its writes need their own native/residual classification; a broad category trigger would incorrectly classify them as job outcomes.

## Required source change and shared ordering

The original job makes multiple separate DB calls. The [source helper](../services/migration-sync/source/poi.sql), called by the job only when `MIGRATION_POI_CAPTURE_ENABLED=true`, commits normalized POI membership, its effective category update and producer-marked outbox entry together. The flag requires the version-4 `places_hybrid_commands_v1` capture installation; a missing or failing helper aborts without fallback. Each place is a separate atomic transaction, not an atomic whole-feed refresh. A crash cannot commit the update without its evidence. Normal deployment/admin writers must not enter that helper implicitly. The source remains the only job owner; no target scheduler or external fetch is enabled.

The helper rejects selected places whose effective and normalized non-POI category sets disagree: silently carrying out the original full-category refresh would exceed this mapper's scope. Outside the selected scope it retains the original full normalized-category refresh. With the flag disabled, the original job path remains unchanged.

Use the same source serialization lock and transaction counter as accepted favorite/like commands. A separate POI stream would not order dependency changes relative to command replay. The classifier must reject unrecognized producers, mixed unsupported mutations and conflicting scope before any target write. It must not treat a caller-supplied HTTP header as job provenance.

Target POI application, residual receipt and stream checkpoint must commit atomically. Missing catalogue identity, foreign/native ownership, a world row or malformed target categories blocks the head without creating a placeholder place. No command may overtake a blocked residual; replayed favorite/like rows must never be reapplied by this mapper.

Enabling a new residual adapter requires an explicit version and consistent baseline; do not relabel existing favorite-only capture history. Bootstrap must align POI membership with the command boundary. Reconciliation must compare only the mapped POI bit plus the other independently owned facts, and fail on an unstable source/target boundary.

## Source defect to fix before activation

The feed client catches network/JSON failures and returns `[]`. The job interprets an empty list as authoritative and removes all currently categorized POIs. This is a static source-code finding, not a claimed production incident. The local source fix distinguishes a valid empty feed from a failed fetch and stops the job before mutations on failure; it also validates the payload and bounds the request with a timeout. Capturing that erroneous empty result more reliably would not repair the source behavior.

## Validation and remaining activation evidence

The [target mapper](../services/migration-sync/src/poi.rs) updates only membership on a locked, strictly owned imported row, with bounded category decoding and native-overlay rejection. The [integration test](../services/migration-sync/tests/poi.rs) covers source rollback, unrelated-category drift, out-of-scope semantics, untagged evidence, truncation rejection, target preservation and shared-stream blocking. All 12 source unit tests pass, covering feed failures versus valid empty data, the atomic-helper call and enabled/disabled job paths; TypeScript checking and scoped ESLint also pass. These checks establish local behavior, not live-source readiness.

Before enabling the residual:

- Exercise the actual POI job with controlled feed results and actual source character-ID columns; prove valid empty input and failed fetch have different outcomes.
- Prove provenance excludes deployment/admin category writes and records rollback/no-op behavior correctly.
- Verify membership add/remove preserves unrelated categories, interaction counts and raw metadata; reject missing, foreign, world and inconsistent rows.
- Interleave accepted favorite/like commands with POI outcomes in one source order; test lost responses, worker restart, duplicate delivery and blocked-head behavior.
- Establish a matching pre-activation snapshot and bounded transaction limits. Test that old adapter versions cannot claim POI continuity.

No deployment, real scheduler run or production load evidence is established by this document. Source application changes are justified specifically by atomic job provenance and feed-failure handling, not by adding sync calls to every write site.
