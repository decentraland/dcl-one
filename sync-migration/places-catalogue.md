# Places catalogue synchronization

## Decision and scope

Extend the unchanged-client source-only stream with explicitly allowlisted Genesis
catalogue rows. Adapter v3 captures `public.places` together with favorites and likes
under one source transaction lock and one repeatable-read baseline. This removes
the requirement to freeze the mapped catalogue's title, categories, visibility and
other row metadata during the pilot. It does not capture arbitrary new IDs, worlds,
all normalized category history, content-rating history, or active provider work.

The [source schema](https://github.com/decentraland/places/blob/2269acb/src/migrations/1660567567429_create-places-table%20copy.ts)
uses timezone-aware timestamps. Capture fixes serialization to UTC and target
projection converts the deployment timestamp to the target UTC timestamp column.
Do not infer source types from older batch-extract comments.

## Why this needs database changes but no Places application PR

Existing source requests, deployment consumers, operator SQL and jobs already write
catalogue rows. A transactional local trigger captures their committed result;
no new client header or HTTP-handler branch is needed. The database installation
adds a trigger to `public.places` and extends required grants. Its serialization
lock affects all writes to that table and must be load-tested before activation.
Source polling remains read-only and never contacts dcl-one during a source write.
A v1/v2 stream cannot be relabelled v3: establish a new reviewed baseline including
catalogue state and history. The target additive migration permits the new adapter
without changing retained generations or positions.

## Proven writer and evidence limits

The [daily POI category job](https://github.com/decentraland/places/blob/2269acb/src/entities/PlaceCategories/tasks/poi.ts)
reads the current POI set and ends each affected place with
[PlaceModel.overrideCategories](https://github.com/decentraland/places/blob/2269acb/src/entities/Place/model.ts),
which updates `places.categories`. The capture fixture reproduces that SQL write
and checks the captured projection and reconciliation. The category fact need not
be recomputed by another scheduler on dcl-one.

This proves capture/application of that job's database outcome, not the deployed
schedule, the production POI feed, every intermediate category-table mutation or
production effect counts. The source job must remain the sole owner. The hot-scenes
job primarily refreshes its service cache; PostgreSQL catalogue capture alone does
not claim equivalent live cache or Redis coverage.

## Mapping and ownership

- Capture full source row metadata except interaction counters and generated
  full-text indexes. Preserve source categories and ranking metadata; set canonical
  target provenance to Places/local. Interaction counts continue deriving from facts.
- Validate Genesis identity, typed visibility/category data, timestamps and bounded
  document size before retaining or applying it. World rows fail the mapping.
- Refuse conflicting target content-owned, local-world or non-migration moderation/
  ranking ownership. Remove only old overlays owned by `migration:places`: the source
  row already carries effective decisions, so retaining stale imported overlays
  would override subsequent authoritative updates.
- Apply scoped inserts, updates and physical deletion in source order, with independent
  catalogue version/tombstone records. Do not delete interaction facts just because
  their catalogue row disappears: only corresponding authoritative source changes
  remove those facts.
- Reconcile both raw metadata and mapped target columns. Equal JSON alone must not
  hide a corrupted category/title column. Baseline absence removes an obsolete scoped
  catalogue row rather than treating a preexisting import as authoritative.

## Removal effects and cutover restriction

The existing target catalogue trigger creates `explore.place_removal_pending`
intents when a place is disabled or removed. Those records remain inert because the
dedicated follower runs no comms cleanup consumer or provider. Do not disable or
silently acknowledge the trigger: retain its lifecycle evidence, then explicitly
reconcile/drain retained intents at ownership transfer before normal services start.
Simply starting the normal runtime on this database is not an approved cutover.

## Validation

The regression suite exercises source category updates, mapped-column drift,
disable/re-enable, physical deletion and reinsert, catalogue tombstones and inert
cleanup intents together with existing interaction snapshot/order/retry tests.
Production load, deployed grants, catalogue key replacement, removal-effect handover
and full public API equivalence remain activation gates.
