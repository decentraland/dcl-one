# Database design contract

Applies to content, Explorer, social and every domain added to `catalyst`: our
application of Richard Fabian's [*Data-Oriented Design*](https://www.dataorienteddesign.com/dodbook/)
to PostgreSQL. Fewer crates is an architecture target; table count follows
the data and its operations.

## Principles and decisions

- **Describe the workload first**: inputs, outputs, row counts, skew, fan-out,
  read/write rates, payload sizes, freshness. Keep expected and measured scale
  apart.
- **Store facts with explicit keys and dependencies.** Separate independently
  changing relationships; declare uniqueness, required values and ranges.
  Give every derived value a named source and update rule.
- **Process useful sets.** Select due work and needed rows in SQL, transform
  bounded batches, keep external validation out of the commit transaction.
- **Move fewer bytes.** Filter and deduplicate keys before fetching large
  payloads; choose indexes from predicates and ordering.
- **Confirm improvements** against an explicit latency, throughput or resource
  budget, write costs included.

## Schema and ownership

1. A schema names an authority and lifecycle. Shared facts have one
   authoritative relation; join across schemas in SQL. Match meanings and keys
   before merging duplicate schemas or same-named tables. Preserve source IDs;
   chain-derived identity includes network, contract and token.
2. Model memberships, friendships, favorites and permissions as keyed
   relations, with explicit acceptance/blocking and local-versus-mirrored
   semantics. No universal entity/attribute table; no JSON document holding
   unrelated, independently updated facts.
3. Keep opaque protocol metadata and ordered auth chains as documents when read
   as a unit; use typed columns or relations for fields used to join, filter or
   update independently. Do not split columns mechanically. Original
   hash-addressed bytes stay immutable in the blob store; never reserialize
   JSON to reconstruct signed or content-addressed input.
4. Start with `PRIMARY KEY`, `UNIQUE`, `NOT NULL`, row-local `CHECK` and
   suitable foreign keys. For every omitted relationship constraint, record its
   invariant, enforcing write path and deletion behavior. Use a cross-schema FK
   only with a shared restore/deletion lifecycle; otherwise keep FKs inside the
   owning schema and define reconciliation. `CHECK` is not a cross-table
   invariant mechanism.
5. Label projections. `active_pointers` and `deleter_deployment` derive from
   deployment ordering and must change atomically with accepted deployments; a
   rebuild must reproduce tie-breaking and deletion semantics. Peer cursors,
   failed retries and `local_entities` provenance are durable: a cache rebuild
   must not discard them or infer local authority from peer data.
6. Keep frequently scanned keys and state small; fetch auth chains, metadata
   and blobs only where needed. Split out a payload relation only after
   measuring row width, updates and joins (TOAST already stores large values
   out of line).
7. Land a DDL change twice once a database is stamped: in `schemas/*.sql` for
   new databases and, when it touches an existing table, as an additive step in
   `schemas/migrations/` (`catalyst schema migrate` applies each once and
   records it in `meta.schema_steps`; `services/catalyst/tests/`). A step
   that drops, renames, retypes or deletes is refused: that change is its own
   reviewed release (record the data-safe release procedure and evidence in the task report).

## Worlds, persistent storage and scene hosting

One worlds subsystem owns deployment/permission lookup, world/player storage
and authoritative scene hosting, on the shared pool with explicit
realm/scene/namespace and player scope. Keep the storage namespace distinct
from a deployment's content hash. Each world scene has a stable identity
(`explore.world_scene_place`) following Places' overlap rule: overlapping
exactly one active scene retains it, including a moved base; overlapping none
or several creates a new identity. Retired identities keep their storage rows.
Genesis storage management requires LAND rights at the requested parcel, using
the published LAND manager index or its configured document source. Without a
document source, only ownership at that parcel grants authority; scene-admin
roles do not grant storage authority.

Storage collaborators are wallets in an active local world deployment's or
active Genesis deployment's `metadata.logsPermissions`; Genesis deployments
accepted through peer sync are eligible too. Per-key and list `/values*` and `/players*`
operations probe the resolved deployment by primary/unique key on every
request; redeployment without the wallet revokes that grant. Collaborator
access excludes clears, `/env*` and `/usage*`.
`GET /world-storage/collaborator` is a rare, user-initiated, self-scoped listing.
It derives membership at read time through `worlds.logs_permissions`, with no
stored projection. The partial GIN `world_scenes_logs_permissions_idx` covers
local world scenes; `deployments_scene_logs_permissions_idx` covers live
Genesis scenes, followed by an `active_pointers_entity_id_idx` probe per match.
Superseded Genesis rows leave the index. One statement counts and pages the
matches by `deployed_at DESC, scene_id` with `LIMIT/OFFSET`.
The function reads raw JSON text so even a stored JSON `\u0000` cannot fail an
insert through an index. Expected traffic is low; measured dataset sizes,
query timings and index write costs must be recorded in the task report.

Genesis collaborator grants also require the configured LAND index to be
complete and fresh. One shared SQL predicate checks the entire scene footprint
for both request authorization and listing, before counting and pagination.
For each parcel, indexed token-history probes select the latest LAND transfer,
estate membership change, and current estate's transfer by block/log position.
Missing ownership or required event timestamps fail closed. Approval and
operator changes do not change this ownership boundary. Transfer away and back
still revokes the old deployment's grant.

The predicate uses the fresh published fold while unrelated events are being
ingested. A relevant LAND transfer, estate membership event or current estate
transfer beyond that published block denies the affected footprint until
publication; approval churn and unrelated parcels do not cause a global
collaborator outage. Publication timestamps and ordinary LAND readiness use
the database clock; the status response observes the cursor and that clock
together. Both
request and listing SQL gate the history function behind a `CASE` membership
check, so a wallet absent from `logsPermissions` does not pay for that probe.

Timestamp repair runs after live ingest and ownership publication, consuming
only the remaining request budget within a 30-second deadline and applying
multiple batches of up to 100 distinct blocks. Cancellation or the deadline
retains committed repair progress. Rejected or partial batches fall back to
counted single-header calls, with periodic batch probes so a transient fault
does not disable batching until restart. An unresolved or hash-mismatched
block stops the live cursor at the preceding verified prefix. The repair due-set and updates are scoped to
the configured LAND/Estate registries. Unknown legacy timestamps keep the
collaborator readiness gate false until repair completes; ordinary ownership
publication remains independent. Verify upgrade lock behavior and backlog estimates before claiming readiness.

The immutable signed entity timestamp minus the protocol's 15-minute maximum
future skew must be strictly later than every relevant ownership event. No
request-time baseline, receipt timestamp, metadata refresh or peer replay
renews the grant. This conservative rule assumes source deployment validation
and ordinary agreement between source-node and chain clocks. A deployment
timestamped within 15 minutes after an ownership change cannot restore the
grant: another deployment with a later signed timestamp is required. Waiting
alone does not restore that old entity.
Unavailable LAND history affects this collaborator grant only; independent
owner, player and scene-server grants retain their own authorization rules.

Genesis scenes need a valid scope too. Use one permission core with explicit
owner/player/host operations. Guests get scoped storage capabilities, never SQL
access or other scenes' environment values. Mirroring cannot overwrite local
saves or grant host authority. Keep live CRDT/tick state in the scene runtime;
persist only what the restart contract requires. Test cross-scene/player
isolation, redeploy, revocation and restart.

Places owns one active identity row per world scene, explicitly bound to its
current `(world_name, entity_id)` deployment. Worlds calls the Places identity
writer in the deployment transaction under the world row lock; storage and
scene discovery resolve that same binding. Catalogue refreshes never mint or
retire identities. Inactive rows retain the UUID but clear the deployment link,
so retained storage does not become reachable from a replacement scene. No
foreign key ties retained identities to replaceable deployment rows; the
transactional writer and partial unique deployment index enforce the binding.
The unique partial `(world_name, entity_id) WHERE active` index enforces one
active place per deployment and serves exact deployment lookups. Its leading
`world_name` column bounds overlap probes and retirement sweeps to the active
scenes in that world (expected up to a few hundred).
World quotas and usage exclude recorded retired identities, and exclude the
parked legacy `world` bucket once that world has an active identity; unknown
other legacy scopes still count. Storage rows remain retained and inaccessible
through new scene scopes. Verify migration and recovery limits and measure quota-query costs before acceptance.

The world discovery listing remains one aggregate per world. Scene listings and
place detail read the shared identities joined to active deployments; neither
adds scene rows to the aggregate catalogue. Upstream mirrored catalogue IDs
remain external IDs, never storage authority. The old
`worlds.world_scene_identity` table is a frozen upgrade source only: creating
the new Places relation copies its UUIDs and retained records during object
creation. This is once per creation, not once for the lifetime of a database:
never drop/recreate the authoritative relation to recover it, because that
would replay the stale legacy snapshot. Existing storage keys are not rewritten,
and migration `0010` stays byte-for-byte stable.

**SQL-level dependency:** Places discovery joins `worlds.world_scenes` and
`worlds.worlds`; Places identity retirement probes `worlds.world_scenes` for
the exact current deployment. Worlds owns those source rows and invokes the
Places identity writer within the same transaction. These cross-schema reads
are part of the shared database contract, independent of Rust crate imports.
The Rust module-DAG check does not inspect SQL and cannot prove these data
dependencies absent. Keep column/visibility and lifecycle changes coordinated
across both domains; no reverse Rust crate dependency or internal HTTP call is
introduced.

Preserve the reviewed method, path, scope and query authorization contract for scene-host storage requests. Handle security acceptance through the private review workflow.

Storage value reads use PostgreSQL by default on every process. The optional
`STORAGE_CACHE_ENABLED=true` memo is only suitable for one storage-serving
process: invalidation is local, and even a primary can return stale values or
environment secrets after a replica writes. `FLEET_JOBS_ENABLED` does not make
that cache coherent. Explicit caching accepts a freshness window of
`STORAGE_CACHE_TTL_SECONDS` and is unsuitable for immediate cross-process revocation.
`STORAGE_CACHE_ENABLED` accepts only `true` or `false`; numeric values such as
`1` and `0` are rejected at startup.

Permission-source outages deliberately return 503 instead of upstream's 401,
without granting access. Transient permission/RPC outages and export database
setup failures (connection acquisition, snapshot preparation or their deadlines)
carry `Retry-After: 1`. Export setup failures occur before acceptance, so they do
not consume the scene cooldown. The header is a minimum retry delay, not a promise
that capacity has recovered; clients should use backoff for repeated failures.
Export concurrency caps continue to bound pending attempts. Invalid export
timeout configuration is rejected at startup; a defensive out-of-range deadline
failure carries no retry hint. Likewise,
missing credentials (including `ENCRYPTION_KEY`) and unsupported EIP-1654
validation require configuration and carry no timed retry instruction. This is
an intentional wire divergence, not a claim of identical error responses.

A storage export (`POST /world-storage/exports`) holds one pooled connection
while reading and serializing world rows and then player rows, never env. The
read-only transaction ends before the final gzip trailer is sent; cancellation,
the export deadline and the raw-size cap bound interrupted producers. `EXPORT_MAX_CONCURRENT_PER_POD` (5) bounds them per
`serve` process and must stay well under `PG_POOL_SIZE` (60): at most a
twelfth of the pool by default, so the request handlers keep the rest. The
caps are per process, as upstream's are per pod.

`export::produce` is rare and user-initiated. Its two row streams use the
namespace primary-key prefix in key order: `world_storage`, then
`player_storage` by `player_address, key`, in one read-only repeatable-read
transaction. A connection is acquired before response headers (unavailable
capacity returns 503), and the producer owns both it and its concurrency slot.
One deadline bounds acquisition, SQL and channel backpressure; dropping the
body cancels the producer. Completion uses a separate channel, so a failure or
panic cannot masquerade as successful HTTP EOF. Transaction-local statement
and idle-in-transaction timeouts match the export budget and reset before pool
reuse. The configured timeout is 1–3600 seconds, defaulting to 120 seconds;
deadline construction also rejects an unrepresentable duration. Scene cooldown
begins only after database setup succeeds, with an atomic admission recheck:
a pre-response 503 releases its reservation without consuming the cooldown.
When cooldown is enabled, only one pending admission per scene can prepare a
database transaction, even if the configured per-scene concurrency cap is higher.
Disabling cooldown preserves concurrent admission up to the configured caps.
Optional selections are
`key = ANY` / `player_address = ANY` filters on that prefix. Not measured:
a scene at its quota (10 MB world plus 1 MB per player).

Scene-host storage requests sign the pathname, allowing `Storage.getValues`
listing queries. Verification still binds the method and declared storage mount;
the unsigned-query trade-off requires security acceptance.

Comms (`services/catalyst/comms`) uses the worlds/content identities and world
permission decisions through shared queries and local calls. World permissions
keep one authoritative representation; social keeps message/call membership
and privacy; comms enforces them and owns its connection/assignment lifecycle.
Persist ACLs, bans and assignment/fencing state needed for recovery under
their owning domains; live connections, presence and media follow the
transport's lifecycle. Presence is not proof of permission. Resolve room
identity from the same active deployment for both clients and authoritative
hosts. Permission revocation must also take effect in the running room, not
merely change a row. Preserve host identity and publish/subscribe grant
distinctions. Test shared-room joins, denied joins, revocation and reconnect
against the real transport.

World allow-lists admit explicit access wallets and members of listed active,
unsuspended communities who are not banned. Admission probes
`worlds.world_access_communities` with `social.community_members` and
`social.community_bans`; their primary keys cover the world/member lookups.
`world_access_communities_community_idx` finds affected worlds by community
for leave, kick, ban, suspension, deletion and mirror-deactivation cleanup.
Expected scale is thousands of grants and tens of room joins per second;
these are estimates, not measured throughput.

## Queries, ingest and derived state

The `/social-api` gateway reads `HTTP_MAX_REQUEST_BODY_BYTES` at startup: the
default is 1 MiB and the accepted range is an integer from 1 byte through 16 MiB.
A declared oversize receives a plain 413 before auth or routing, with the actual
response CORS headers and `Connection: close`. Social handlers defer body reads
until their existing auth gate and count streamed bytes against this cap. The
source schema boundary returns its own 400 for a failed read; multipart and raw
mute readers retain the source generic 500 for a global stream overflow.
The independent multipart total remains 1 MiB, even with a larger HTTP cap;
thumbnail bytes have a separate 1 KiB through 500 KiB range. A multipart limit
failure returns 400. The process's existing outer drain still handles bodies
dropped without a handler read, under its shared upload budget; per-social
unread-stream limiting and close timing remain outside these consumed-body
checks. Other domains keep their existing body and drain policy.

Community SNS egress reuses the legacy publisher already held by the referral
component and the existing SQL pool. The raw event precedes the inbox consumer's
opt-out filter. Its audience lookup reads membership in pages of 100 ordered by
role and join time; recipient messages contain at most 100 wallets, and the SNS
adapter sends at most ten messages per request. These are paginated live reads,
without a transaction snapshot across pages. HTTP producers start the broadcast
in the background; voice START awaits it. Publisher and audience failures are
logged and do not undo the mutation. There is no durable egress queue or replay
guarantee. A community deletion sends the content-violation event when applicable,
then the member deletion batches, then its realtime deletion update. The inbox
writer remains a separate consumer and applies subscription preferences.

Event notification delivery is owned by Social in
`social.event_notification_intents`. Explore inserts an event transition snapshot
through qualified SQL in the same transaction as the event change; Social owns
recipient progress and completion. This shared-database edge has no reverse Rust
dependency or internal HTTP transport. No event foreign key is used: committed
host notices must survive removal of a mirrored catalogue row.

The lifecycle scanner considers live occurrences in a rolling one-hour catch-up
window with a local audience, including on first deployment. Separate scan
budgets prevent reminder work from starving starts. Stable occurrence keys and a unique queue index
deduplicate repeated scans, including late approvals and equal due times. A
recipient page commits its cursor with notification and email-outbox inserts.
Completed queue rows have 30-day retention, exceeding the catch-up window;
bounded cleanup retains unfinished work. Stale reminders and ended occurrences
are suppressed before delivery. Cleanup has a bounded opportunity before dispatch,
including when there is a backlog. Claims rotate queue order durably before page
work; transaction-local database timeouts bound slow queries and lock waits.
Recipient branches are bounded before merging the page. Notification timestamps
use the delivery clock so incremental polling sees catch-up deliveries; source
timing remains in metadata. The worker uses a five-second dispatch budget rather than a
fixed number of audience pages. Independent three-second scan budgets and
two-second scan statement timeouts allow more time for catalogue discovery than
for recipient pages. Scan timeouts and budget exhaustion emit error-level
diagnostics without activating job backoff; other scan failures reach job error
reporting after dispatch. Cleanup failures and budget exhaustion emit error-level
diagnostics without stopping delivery. Committed stale or
obsolete suppression logs the intent and reason, including expiry after lock
contention. Community opt-out UUIDs are canonicalized on write and lookup;
event and community delivery recognize existing non-canonical UUID scopes too.
Lookup binds optional UUID parameters parsed in Rust, preserving opaque legacy
scope IDs without relying on guarded casts of SQL parameters.
Catalogue-scale capacity is not established by
bounded result sizes alone. See the
[event notification ADR](docs/adr/2026-11-01-event-notification-delivery.md).

Event RSVP state in `explore.event_attendance_local` is authoritative for that
wallet: cancellation markers override mirrored attendee previews in reads,
personal filters and attendee listings, and survive mirror refreshes. Counts
apply a signed local delta to the mirrored aggregate, deduplicating wallets
known in the mirrored preview. The preview is incomplete: an additional local
going wallet may already be included in the upstream aggregate. Without an
authoritative upstream membership list, the combined total remains an estimate;
it does not guarantee exact deduplication outside the known preview. Local-only
events do not have this ambiguity.
The read projection materializes the normalized preview once per event row and
excludes SQL-null preview entries. Local membership probes retain the
`event_attendance_local` primary-key lookup; cancellation and going deltas share
that preview instead of expanding its JSON separately for every local RSVP.

Event authoring distinguishes generated posters from explicit images and clears
inside the existing event document. Generated images may follow location
changes; an explicit clear must survive later edits. Missing profile or place
lookups must not erase stored facts on an unrelated edit. Location-derived facts
must be refreshed or cleared when their source location changes. This metadata
does not establish owner identity or grant permissions. Existing nonempty posters
without authoring provenance remain explicit: their source cannot be inferred
reliably from the URL alone. There is no historical provenance backfill.
The internal `_image_source` key can travel with raw document copies. Public API
and export paths must use an explicit field projection instead of publishing the
stored document unfiltered. `EventRecord` omits this key, and notification
snapshots use their own allowlist.

For Shop: keep explicit asset/chain/order identities, separate accounting
facts from rebuildable projections, bound reconciliation, use exact currency
units, reserve atomically with external work outside the transaction. The
MANA-only LAND/resale rule holds through authorization and settlement; a
stored client-supplied sale category never establishes eligibility.

- Share the pool and use qualified SQL joins. Return the fields the consumer
  needs; avoid per-result round trips. Preserve API ordering, collation,
  pagination and empty/error distinctions when moving Rust work into SQL.
- Keep the bounded `UNNEST` ingest and deterministic pointer locks. Bound bytes
  and transaction duration as well as entity count; measure pool and lock
  waits under reads. Advance durable progress only after data or its retry
  record is durable; keep duplicate/out-of-order/resume tests.
- Poll indexed, bounded due sets for retries, mirrors and notification
  delivery. Keep exhausted-row cleanup and no-auth behavior explicit. With
  multiple workers, use a short atomic claim with lease/recovery semantics; do
  network work outside the transaction; preserve idempotency on redelivery.
- Give every cached or materialized result a source, freshness budget,
  invalidation and rebuild path. Notifications may wake a reader; durable
  delivery needs a transactional record and a resumable consumer. Keep
  triggers small and count their cost in ingest measurements.
- Each index names its query or integrity constraint in the SQL comment beside
  it. Match filter, sort, expression and keyset tie-breaker; check the actual
  prepared plan. Measure storage/WAL/update cost before adding or removing an
  index; a tiny lab run with zero scans is not evidence to remove one.
  Partial-index predicates match the query; current time belongs in the
  due-work query, never in a moving `now()` index predicate.
- Prefer stable keyset pagination for new feeds and mirror scans; keep
  required offset APIs. Define tie-breaking, replay windows and deletion
  handling; a cursor alone does not give snapshot consistency.

## Measurement and acceptance

Use read-only PostgreSQL catalogue and statistics queries through psql for a
baseline (for example `pg_stat_user_tables` and `pg_stat_user_indexes`). Capture
a before/after pair over the same workload window; counters are cumulative and
row counts are estimates. For a
query/layout change record revision, PostgreSQL version, hardware, dataset
size/distribution, concurrency, cache state, query/parameters, repeated
p50/p95/p99, throughput and resource cost. Use `EXPLAIN (ANALYZE, BUFFERS,
WAL, SETTINGS)`; `ANALYZE` executes, so measure writes on a disposable
database. Buffer hits are not CPU cache hits. Before accepting a slice, prove
wire, authorization, concurrency and recovery behavior and state the
improvement and trade-off; smoke-scale bootstrap runs are not benchmarks.
Record missing measurements explicitly.

## Bootstrap, schema versions and local writes

`catalyst schema apply` bootstraps `catalyst` from the binary's rendering of
`schemas/`; it never replays third-party schema histories or dumps. Startup
verifies schema identity and version and fails clearly on a mismatch; it never
resets persistent data. Drop and re-apply disposable development databases. A
database holding local authority changes only through a data-safe release
(record the data-safe release procedure and evidence in the task report): additive steps under
`schemas/migrations/` applied once by `catalyst schema migrate`; anything that
drops, renames or retypes is its own reviewed release under `schemas/releases/`.

The binary requires schema version 3. A database stamped version 1 first takes
`schemas/releases/v2-shop-schema.sql` with all writers stopped and a backup
restore verified. A version-2 database then takes the private-voice release:

```sh
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f schemas/releases/v3-private-voice-pending.sql
catalyst schema migrate
catalyst schema status
```

Keep every writer stopped through that sequence. The release audits pending
wallets across caller and callee roles after normalization and refuses overlapping
or non-normalized pending state without changing data. It drops only the two
caller/callee uniqueness constraints and stamps version 3 in one transaction.
The canonical object pass of `schema migrate` creates the nonunique
`private_voice_chats_caller_idx` and `private_voice_chats_callee_idx`; verify
both valid indexes before traffic resumes. The release does not delete, merge
or rewrite calls or participants. A failed transaction preserves the version-2
stamp and constraints. Both older and newer binaries refuse a mismatched stamp.

Retained media can coexist with new pending calls in version 3. Once such data
exists, reinstating the former uniqueness constraints is not a general rollback.
Quiesce and use a reviewed forward fix or restore; never delete or deduplicate
media calls to force those constraints back. The owned populated upgrade tests
exercise release, migration, ledger and replay. Deployment backup, restore and
rollback evidence remains a separate gate.

Peer sync is not a backup of local state. Keep local write paths disabled until a backup restore of the node's database, blobs and credentials has been demonstrated. Record the restore and rollback evidence in the task report before enabling writes. Development and commit builds sharing a database must agree on its schema; do not reset or silently migrate shared state to fit a working tree.


## Social realtime notification workload

Social keeps the notification scope, generation UUID and 24-hour expiry on
`social.communities`; the all-null/all-present check couples their lifecycle.
These facts preserve the START audience across process restart and visibility edits.
Comms still owns media and participant presence. A 45-second default poll reads
tracked generations in ordered PK pages of 64, using the partial
`communities_voice_notifications_idx`, and updates or clears a row by PK plus
its generation. Generation CAS prevents stale metadata cleanup from erasing a new
START. No SQL lock spans a media or NATS await.

`social.event_payloads` is an optional immutable transport cache for community
voice metadata whose event exceeds the existing 64 KiB NATS admission limit.
The writer inserts opaque binary protobuf bytes under a random UUID, capped at
512 MiB and expiring after 24 hours. Normal events require no insert. Receivers
perform one fresh-only PK lookup for an oversized event; separate snapshots cannot
overwrite each other. Private-call credentials never use this relation. The
`event_payloads_expiry_idx(expires_at,id)` supports ordered deletion of at most 64
expired rows with `FOR UPDATE SKIP LOCKED` per polling pass. This relation does not
record delivery, retry, or replay work.

The expected hot set is tracked active voice communities plus oversized events
published during the preceding day, rather than all communities or subscribers.
Membership fanout uses set probes of at most 256 subscriber addresses per SQL
statement. A community voice update snapshots eligible subscribers and completes
all membership reads before emitting. Any failed page suppresses the entire
member-scoped update; a public start instead clears all membership annotations and
announces to the whole snapshot with `isMember=false`. A public end skips
membership reads. The snapshot and membership set each hold at most one entry per
eligible subscriber. Production active-room counts, oversized-event frequency and
a 512 MiB payload's memory/latency cost have not been measured; the byte limit is
an admission bound, not a demonstrated capacity. Operators can observe transport
drops through `social_event_delivery_dropped` and retain the existing SQL/job instrumentation.

Private-call accept, reject and end RPCs first run the parameterized scalar query
`SELECT $1::text::uuid` through the existing pool. This adds one SQL round trip per
RPC so PostgreSQL controls accepted UUID spellings and syntax-error messages before
the stricter media identifier check. It reads no application row, takes no row
lock and needs no index. HTTP keeps its own canonical UUID validation and calls
the typed operation directly. Query-only observations are recorded in `METRICS.md`;
they do not establish equal latency with the source's combined lookup.

## Social community ranking and compliance workload

Community ranking reads current membership, posts, places and thumbnail facts in
pages of 100 communities. Five lifetime counters on `social.communities` retain
photos, ended events, event attendees, ended streams and ended voice chats. The
ranking job updates local authoritative communities only. Mirrored ranking scores
remain upstream facts. `ranking_tracking_since` bounds local event occurrences
eligible for collection; zero counter defaults do not reconstruct history. An
authoritative cutover must import the lifetime counters, collection watermark and
local producer receipts together before enabling local ranking ownership.

`social.community_ranking_events` retains local producer receipts with primary key
`(kind,event_key,community_id)`. One photo may contribute to several communities;
receipt insertion and counter changes share a transaction. Receipts have no expiry.
External decoded events preserve the upstream consumer's repeated increments,
including its finally-acknowledge behavior on handler failure. The receipt relation
does not turn that transport into an exactly-once consumer.

New camera uploads start with `ranking_published=false`. After both blobs are
written, one transaction marks the row and applies local photo contributions.
Existing photos receive `true` during migration, preserving the history boundary.
The existing job repairs at most 100 pending photos per pass, reading the partial
`images_ranking_pending_idx`; it checks both blobs before taking SQL locks. Deletion
does not decrement lifetime counters. A failed best-effort publication followed by
photo deletion before repair can lose the contribution.

Community create and edit compliance share `social.community_compliance_cache`
through the existing pool. Its primary key is `(model,input_hash)`, where the hash
is 32 bytes of SHA-256 over the source's name, description and image-digest
serialization. Rows store the whole validated provider result, including rejected
content, for two hours; they do not store the input text or image. Reads require a
fresh expiry. The expiry index supports removal of at most 1,000 expired rows per
hour, rechecking expiry to preserve concurrent refreshes. Provider failures are not
cached. Cache-read SQL failures remain handler failures; a cache-write failure is
reported by the source's compliance-unavailable branch.

The compliance adapter builds one lazy Outbound client per community State. Its
600-second whole-response deadline and 4 MiB response cap are explicit bounds.
The pinned OpenAI SDK clears its 600-second timer at response headers and relies
on the runtime's body inactivity timeout, so these bounds have a narrower scope
than unrestricted SDK responses. The POST/PUT source contracts exercise 20 mocked
provider cases; they do not establish exhaustive provider equivalence.

## Shared social registry profile facts

RPC and community profile readers share `social.registry_profile_cache` through
one existing pool. The primary key is the source registry cache key. A single
read writes the requested wallet key; a batch writes the public `avatar.userId`
key. Public profile identity never replaces the actor used for authorization or
relationship storage. Batch reads retain cached-first order and newly fetched
aliases, including duplicates admitted by the pinned source registry.

Each successful write retains the minimal public profile for 600 seconds, measured
by the database clock. Reads ignore expired rows even if cleanup is delayed.
Missing or invalid profiles are not cached. Accepted content updates do not
invalidate these facts; the source retains its previous value until expiry. The
six V1 profile-enriched community reads (community listing, members, bans,
community requests, member requests and posts) propagate cache-read failures to
their handler error boundary. Consumers that intentionally use the best-effort
profile wrapper retain that behavior. V1 members, bans and community requests
omit unresolved profile rows after database pagination, preserving the database
total; their V2 responses retain those rows without profiles. Post-author and
strict single-owner behavior remain separate. Cache writes are best effort: a
batch first uses one set write and retries individual keys if it fails, so one
failed key does not prevent successful keys from being retained.

The workload consists of indexed key/`ANY` reads, existing active-pointer/profile
lookups on misses and bounded page writes. No external await occurs inside an SQL
transaction. `registry_profile_cache_expiry_idx(expires_at,profile_key)` supports
the existing one-second leased expiry job, selecting and deleting at most 1,000
expired rows per pass. Delayed cleanup affects storage growth, not freshness.
Production new-key rates and sustained reclamation capacity remain unmeasured.
An authoritative cutover must import cached values with their remaining TTL or
drain the source cache before switching readers; this runtime procedure is not
established by the local reader and migration tests.

A newly inserted referral performs one recent-invitation query by referrer,
ordered by `created_at DESC` with a result limit of two. The existing
`referral_progress_referrer_idx(referrer,status)` supports its equality filter;
the database sorts matching history to select the two newest rows. The result is
bounded, while the scanned history grows with that referrer. A query failure after
the insert retains attribution and reaches the source handler's error boundary.
Configured timing/IP Slack calls occur after insertion and the hundred-invite
alert after the grant and accepted notification; no external await holds an SQL
transaction. Same-referrer retries exit before recent reads or alert delivery.

Registry cache keys retain the requested spelling, including case; batch key
uniqueness is case-sensitive. Only the physical content-pointer lookup normalizes
case and resolves one document per normalized pointer. Batch cache-hit matching
compares the normalized public user ID to each requested key lowercased, while
single-read errors retain the original requested key. This keeps mixed-case cache
facts separate from authenticated wallet normalization and from the public ID.

Community analytics reuses the configured Social analytics adapter and Outbound
client. It adds no SQL storage, delivery receipt, retry queue or transaction that
waits for HTTP. Join and opposite-request acceptance report the removed request
ID when one exists; no ID property is sent when none exists. Making a community
public reports the IDs of removed non-banned `request_to_join` rows, including
historical status values, as the pinned source adapter's acceptance query does.
That write can return one ID per matching request, so its returned data and
analytics payload grow with that community's matching requests. The regression
covers two accepted rows and one retained banned row; sustained large-community
load is unmeasured. Analytics fetch failures do not change the mutation response.

Client community writes retain the pinned Social source's commit boundaries.
BAN commits membership removal, then removal of that wallet's likes, emits
analytics, commits the ban, and removes its requests. A later failure preserves
completed stages; voice eviction, member-status publication and the ban
notification follow successful request removal. These are separate statements
on the existing pool, with no transaction around external delivery. Updating a
community commits its fields before thumbnail and place changes. Place removal
and insertion commit separately. Changing privacy to public then accepts
non-banned join requests in its own transaction: membership insertion and request
deletion roll back together on failure, while the earlier privacy change stays.
The atomic join-and-remove-requests operation remains separate from these
workflows. Trigger-injected regressions cover one affected member, like and
request, plus place replacement; they do not measure concurrent cutover or
large-community workloads.

## Private voice pending and media workload

`social.private_voice_chats` records call identity and both parties. Absence of
participant rows means a pending intent; accepting inserts both participants
atomically. Only pending creation commits a call without participants. It
normalizes and sorts both wallets, acquires their transaction advisory locks,
and probes pending rows in a fresh statement under explicit `READ COMMITTED`,
including when the role default is stronger. Those locks enforce one pending
per wallet across both roles. Pending expiry, rejection and disconnect delete
whole pending calls; media cleanup deletes whole media calls and cascades their
participants. No path removes the last participant while retaining its call.

Live media presence still refuses a new Social call. Historical or expired media
can coexist with a new pending intent until its own lifecycle removes it. The two
nonunique address indexes support the caller/callee membership probes without
constraining that history. Pending creation does not delete media. Expected
traffic is low-rate two-wallet writes and indexed existence probes, with at most
one pending per normalized wallet; retained media depends on its connection and
cleanup lifecycle. Capacity and production latency are not established by the
functional tests.

Community detail, listing and member-request reads fetch voice status separately
through the existing comms `VoiceDb` on the same pool and presence configuration.
An ordinary page uses one primary-key-bounded bulk read for its community IDs;
core community query errors remain fallible. Voice-query errors become idle
statuses, matching the pinned companion's SQL catch. Active-room filters and the
active-room endpoint first read all active rooms; an empty or failed voice read
returns an empty response before community SQL. This all-room query still grows
with stored voice presence. These local SQL catches do not describe direct
HTTP-provider errors in the separate Social deployment.

Signed community listings, members, bans and member-request summaries propagate
relationship-query failures through their handler boundary on the existing pool.
Members and bans read relationship facts before V1 profiles, including empty
pages; anonymous listing and member reads omit those relationship queries.
Community-request rows carry `friendshipStatus: NONE` without a relationship
read. Ordinary signed community pages use the existing accepted-friend read and
at most three matching member addresses per community; no index or storage was
added. The legacy best-effort friendship-status wrapper remains available to
its existing consumers outside these community reads.
