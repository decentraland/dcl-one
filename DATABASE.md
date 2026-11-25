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
   A changed body of a hash-named function (`content.deploy_entity_<hash>`,
   `content.unpublish_scene_<hash>`, `social.world_authorization_started_<hash>`,
   `social.world_secret_refund_<hash>`, the `explore.*_<hash>` functions the
   generated columns call) is a new function; `migrate` creates it and
   leaves the old one for binaries still calling it. `catalyst schema status`
   lists the old ones as superseded after migrate (run before `migrate`, that
   includes the one the running binary still calls), never one a column
   default, a generated column or an index still calls (`pg_depend`; dropping
   it would take the column or the index with it); drop the listed ones by
   hand once no process runs an older binary.
   A step that adds a column calling such a function creates it first with
   `CREATE FUNCTION`, skipped when the database has it. A body that computes
   the same values at a lower cost is the one in-place change: the binary
   hashes it under a new name for fresh databases, and a step replaces the
   shipped name's body with it (`CREATE OR REPLACE FUNCTION`), so the
   generated columns of a migrated database take the new cost without a
   rewrite; a test pins the replacement to the binary's definition and a
   test shows both bodies agree on every input a generated column can pass
   them (`0033_explore_recurrence_one_pass`; `raw` is NOT NULL, and a SQL
   NULL `raw` is the one input where the `live_until` bodies differ).
   The `deleter_deployment` repair runs once per database and repair version
   (`content.system_properties` key `deleter_repair_version`, only ever
   raised), at a sync leader's bootstrap or a non-syncing process's startup.
   During a rolling deploy an older binary still writing can leave rows the
   repair's rules would change after the marker is set; the next sync leader
   bootstrap repairs them, or restart a sync leader once the rollout is done.
   The repair marks each snapshot window whose rows it superseded with
   `content.snapshots.outdated` (step `0026_content_snapshots_outdated`, a
   constant default without a rewrite; the table holds a few dozen rows, one
   per generation window). The generator's tick scans for the flag and
   regenerates those windows; the row keeps its real `generation_time`, which
   `/snapshots` publishes as upstream does.

Content sync looks up peer snapshot receipts by primary key in batches during
bootstrap and periodic refresh. A receipt records the entity types already
processed. Unfiltered coverage uses a null array. Migration 0061 initializes
older receipts to an empty set. On the first sync pass after upgrade, the
leader can redownload and parse every advertised peer snapshot with a legacy
receipt, including on unfiltered nodes. The existing snapshot concurrency
bounds parallel work, and the deployer still skips entities already present.
Writes extend coverage and a peer's cursor advances after its receipts are durable.
Peer downloads use temporary staging. The blob collector can reclaim old
peer snapshot files once their final owning reference is gone. A local
published snapshot continues to retain its blob.

New local deployments require an array when `content` is present. Peer replay
reads an explicit null as an empty list for compatibility with legacy manifests.
Both paths accept at most 25,000 declarations per entity, counted before hashes
are deduplicated. An older manifest above that limit can remain stored on the
node that accepted it. A new deployment or a replay of that manifest fails
validation. The `content_sync_manifest_rejections_total` counter reports peer
rejections with the reason `declaration_limit` or `invalid_cid`.

## Worlds, persistent storage and scene hosting

One worlds subsystem owns deployment/permission lookup, world/player storage
and authoritative scene hosting, on the shared pool with explicit
realm/scene/namespace and player scope. Keep the storage namespace distinct
from a deployment's content hash. Each world scene has a stable identity
(`explore.world_scene_place`) following Places' overlap rule: overlapping
exactly one active scene retains it, including a moved base; overlapping none
or several creates a new identity. Retired identities keep their storage rows.

Partial batches for one entity hold a session advisory lock from their first
pending-upload/replay read through the staging or committed-publication decision.
The guard owns one connection from the shared pool, and every guarded query and
short transaction uses it; this supports a pool with one connection. A permit
budget shared by CoreState bounds partial-session owners to
`min(upload_limit, max(1, pool_max / 4))`. Permit waits use the existing request
timeout; failed lock attempts return both checkout and permit before retrying.
Staging commits remain independent of blob persistence. Cancellation or
uncertain unlock closes the session. Connection ownership precedes the name
deny-list refresh mutex, and ordinary readers return their checkout before
waiting for an active refresh. Publication releases the session before its
existing synchronous postcommit access revocation and other effects, which still
finish before the response. Cleanup retains its independent store lock and
expiry rules. See the
[session ownership decision](docs/adr/2026-11-16-worlds-partial-upload-session.md).

Partial (multi-request) world publishes stage in `worlds.pending_scenes` and
`worlds.pending_scene_files`, with no world row until the completing batch's
publish transaction consumes the upload and writes its
`worlds.completed_scene_uploads` receipt. Upload counts and staged bytes are
admitted under advisory locks (deployer, world, node budget); the per-world
count and byte sums read `pending_scenes (world_name, created_at)`. The blob
collector keeps every file a pending row names, and a leased job reclaims
expired uploads under the exclusive store lock, unlinking only files a
batch of that upload wrote (`pending_scene_files.written`). Local content
deploys (`POST /entities` and partial publication) and world scene deploys
take that lock's shared side in their write transaction and re-check every
file they name, so an unlink batch never removes a file such a deployment
references; a content deploy waits at most 5 s for it and answers 503 past
that. The peer-sync flush (read-through's too) takes the shared side in its
batch transaction and probes the batch's distinct files there; an entity
whose file is gone is left out and recorded in `content.failed_deployments`
for the retry worker, and the rest commits. So the unlinks that skip the age
floor never remove a file a content deployment commits: the history prune
and content's partial-upload cleanup, which deletes up to 20 expired
uploads' `content.pending_deployments` rows and unlinks the files they wrote
(`pending_deployment_files.written`) that no reference names in one
transaction under the exclusive lock. It reads the world scenes' content
lists before taking the lock and, under it, only the rows written since; a
lock still busy after 30 s ends the run, and uploads that wrote no file go
without it. An upload a newer deployment overtook is released the same way
within its request, which takes the lock only if it is free and otherwise
marks the upload expired for the cleanup's next run. Builder's item file and
project blob uploads also take the shared side (waiting at most 5 s, then
503) before checking whether a blob is stored and hold it until the `builder.uploaded_contents` row (and, for an
item file, the `builder.item_contents` row) naming it commits. The other
writers that commit a row naming a blob do the same through
`gc::store_for_row` (the shared side, at most 5 s, then the blob stored
unless present, in the row's transaction): the asset-pack catalogue build
(which re-checks every walked file and stores a removed one again), the
camera-reel import, community thumbnails and contents, MLS blobs, cached
profile images, worlds settings thumbnails, event posters and outfit
thumbnails. A worlds mirror pass re-checks the blobs its rows name under
the shared side and fails when one is gone, for the next pass to fetch
again. Camera-reel uploads and player report evidence commit their row
before storing the blob, so an unlink batch's reference read keeps it.

`worlds.thumbnail_aliases` records one immutable SHA-256-to-CID mapping for
each distinct mirrored thumbnail that the raw-leaf CID cannot name, including
images larger than one 256 KiB chunk. The mirror checks the SHA-256 digest
before inserting the mapping under the same store lock as the settings write.
The mirror and the cutover importer record aliases even when the world keeps
its local settings. The importer uses the verified blob inventory. Legacy
hex reads probe the primary key after a raw-leaf miss. The collector uses
the alias CID index to protect imported aliases and aliases that predate
expiry tracking. Other aliases protect their blobs for 30 days after
renewal. The mirror renews an alias at most once a day while the source
still publishes it, including when local settings take precedence. A
world's current thumbnail also has its own reference. Once an alias
expires, the blob is collectable if no other reference keeps it. The
hash-to-CID mapping remains, and the content denylist can withhold the bytes.

Read-through copies (explore's satellite, manifest and poster mirrors, the
asset catalogue pull) take no lock: a removed blob is fetched again on its
next read.

Genesis storage management requires LAND rights at the requested parcel, using
the published LAND manager index or its configured document source. Without a
document source, only ownership at that parcel grants authority; scene-admin
roles do not grant storage authority.

Adding a Genesis place to a community reads the same LAND state: the place
belongs to the owner of its base parcel (or of the estate that holds it), never
to the owner its scene metadata names
([decision](docs/adr/2026-11-21-community-genesis-place-owner.md)).

Storage collaborators are wallets in an active local world deployment's or
active Genesis deployment's `metadata.logsPermissions`; Genesis deployments
accepted through peer sync are eligible too. Per-key and list `/values*` and `/players*`
operations probe the resolved deployment by primary/unique key on every
request; redeployment without the wallet revokes that grant. Collaborator
access excludes clears, `/env*` and `/usage*`.
`GET /world-storage/collaborator` is a rare, user-initiated, self-scoped listing.
It derives membership at read time through `worlds.logs_permissions_decoded`, with no
stored projection. The partial GIN `world_scenes_logs_permissions_decoded_idx` covers
local world scenes; `deployments_scene_v_logs_permissions_decoded_idx` covers live
Genesis scenes (content stores metadata as `{"v": metadata}`, so it indexes
`entity_metadata -> 'v' -> 'logsPermissions'`), followed by an `active_pointers_entity_id_idx` probe per match.
Superseded Genesis rows leave the index. One statement counts and pages the
matches by `deployed_at DESC, scene_id` with `LIMIT/OFFSET`.
The function decodes JSON escapes as `JSON.parse` does and leaves out an entry
carrying `\u0000` or a surrogate escape (no address), so no stored JSON value
can fail an insert through an index. It reads the escapes as Unicode, so the database
must use the `UTF8` server encoding (the one `catalyst` supports): under another
encoding a non-ASCII escape would fail the insert or the index build. `schema apply`
and `schema migrate` check the server encoding before any schema changes. A database stamped before it keeps the
superseded `worlds.logs_permissions` and its two indexes, and a database migrated
before the `v` path keeps `deployments_scene_logs_permissions_decoded_idx` on the
unwrapped path; `catalyst schema status` reports none of them. Their release drops
them. Run it with the administrative role, after `catalyst schema migrate` created
the decoded function and its current indexes and every replica runs a binary that
queries them (an older `serve` calls the old
function from its storage grant and `GET /collaborator`, which fail after the
drop); it refuses a database without the decoded function and both its indexes
built and valid, and drops the old indexes concurrently, outside a transaction
(rerun it if interrupted):

```sh
psql "$DATABASE_URL" -X -f schemas/releases/worlds-logs-permissions-retired.sql
```

The object pass builds `deployments_scene_v_logs_permissions_decoded_idx` on
`content.deployments` in the migrate transaction, not concurrently, so writes to
that table wait for the build (unmeasured on production). To build it outside
that window, run beforehand, with the same definition as `schemas/20_worlds.sql`
(the function first, then the index, which the object pass then skips):

```sql
CREATE FUNCTION worlds.logs_permissions_decoded(text) ... ; -- as in schemas/20_worlds.sql
CREATE INDEX CONCURRENTLY deployments_scene_v_logs_permissions_decoded_idx
    ON content.deployments
    USING gin (worlds.logs_permissions_decoded((entity_metadata -> 'v' -> 'logsPermissions')::text))
    WHERE entity_type = 'scene' AND deleter_deployment IS NULL;
```

A failed concurrent build leaves an invalid index of that name: drop it before
retrying.
Expected traffic is low; measured dataset sizes, query timings and index write
costs must be recorded in the task report.

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

A Genesis scene's storage scope (realm `main`) is its effective base
parcel: the declared `metadata.scene.base` when it is one of the scene's parcels,
else its least parcel. The resolver reads it at `entity_metadata -> 'v'`, where
content stores the metadata as `{"v": metadata}`.

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
through new scene scopes.

The partial `world_scene_place_retired_identity_idx` on
`(world_name, (identity::text)) WHERE NOT active` bounds the quota exclusion to
retired identities in the requested world. It keeps the UUID-to-text comparison:
legacy namespaces need not parse as UUIDs. The object pass blocks writes to the table until the migration transaction
commits. For a concurrent build ahead of migration, use the definition from
`schemas/21_world_scene_places.sql` and verify both `pg_index.indisvalid` and
`pg_get_indexdef(indexrelid)`. Migration, bootstrap and status check only the
name. Drop and rebuild any invalid or mismatched index before migration.
See the [quota plan measurements](METRICS.md#wallet-presence-and-world-quota-lookups).

The world discovery listing remains one aggregate per world. Scene listings and
place detail read the shared identities joined to active deployments; neither
adds scene rows to the aggregate catalogue. Upstream mirrored catalogue IDs
remain external IDs, never storage authority.

World publication, undeployment, settings (including spawn), access and block
edits, and owner reconciliation mark one row in `explore.world_place_updates`
in the source transaction.
This relation holds at most one pending row per changed world. Coalescing takes
the pending row's lock without losing its original queue position, so a worker
cannot consume a mark before its source update commits. The leased
`places-derive` job takes up to 64 oldest entries with `SKIP LOCKED`.
Full batches are followed immediately by another check. Partial or empty
batches resume the two-second interval. The update or removal of the
aggregate and the consumption of its mark commit together. The worker reads
world and scene rows without taking row locks. A failed refresh rolls back
both effects. Consecutive refresh failures back off to at most 30 seconds.
A full reconciliation runs at its configured interval and can
delay processing queued edits while it runs. The local projection and mirror
upserts of worlds share an advisory transaction lock. The mirror checks for
local authority after acquiring the lock, so it cannot reinsert a world
displaced by the local projection. The lock also orders work during lease changes.

The old `worlds.world_scene_identity` table is a frozen upgrade source only: creating
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

Single world and player storage values (never env values or listings) are
read through a per-process LRU, as upstream: on unless `STORAGE_CACHE_ENABLED`
is exactly `false`, at most `STORAGE_CACHE_MAX` entries (8000, least recently
used evicted first), each fresh for `STORAGE_CACHE_TTL_SECONDS` (60) after it
was cached, values over `STORAGE_CACHE_MAX_VALUE_BYTES` (32768) never cached,
misses never cached. Invalidation is local to the process that wrote: with
several storage-serving processes, a read may return a value up to the TTL
old after another process's write or delete, as on upstream's replicas.
`FLEET_JOBS_ENABLED` does not make that cache coherent. A deployment needing
immediate cross-process freshness sets `STORAGE_CACHE_ENABLED=false`.

Permission-source outages deliberately return 503 instead of upstream's 401,
without granting access. Transient permission/RPC outages carry
`Retry-After: 1`. The header is a minimum retry delay, not a promise
that capacity has recovered; clients should use backoff for repeated failures.
Unsupported EIP-1654 validation requires configuration and carries no timed
retry instruction. `ENCRYPTION_KEY` is required: `serve` refuses to start
without it. This is an intentional wire divergence, not a claim of identical
error responses.

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
transaction. As upstream, admission answers 200 and starts both the scene
cooldown and the `EXPORT_TIMEOUT_SECONDS` timer before any database work; the
producer then acquires the connection, owning it and its concurrency slot. A
connection, snapshot or query failure after the headers cuts the stream (an
HTTP body error, never a valid gzip trailer) and still consumes the cooldown.
The timer bounds acquisition, SQL and channel backpressure; dropping the body
cancels the producer. Completion uses a separate channel, so a failure or
panic cannot masquerade as successful HTTP EOF. Transaction-local
`statement_timeout` and `idle_in_transaction_session_timeout` are
`PG_COMPONENT_STREAM_QUERY_TIMEOUT` (120000 ms; 0 disables them) and reset
before pool reuse. The export settings take any value, as upstream's: a zero
concurrency cap answers every export 429, a zero timeout or size cap aborts
every stream, and a timeout too large to represent sets no deadline.
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

Selected room ownership uses the existing `social.archipelago_v4_assignments`
realm primary key and global connection-epoch sequence on the shared pool. Signaling
registration writes no owner row. Expected work is one candidate allocation per
changed eligible legacy transport, indexed single-wallet ownership reads, and
conditional claim, renewal and assignment publication for the positively selected
Pulse identity. Steady owned renewal is due after 30 seconds; the lease lapses after
90 seconds. Native selection requires no signaling socket. Maintenance retries unknown,
borrowed, conflicting or unfinished targets after five seconds; healthy exact owned
targets become due 30 seconds after a confirmed local renewal. Each attempt can perform
Pulse positive and retirement requests plus wallet-before-realm SQL settlement, even
when an old source no longer responds. Due targets rotate in bounded batches refilled
on capacity notifications, independently of the one-second fallback wake. Maintenance
has at most 128 queued/running items, 128 lifetime mutation credits and 16 running
items within the shared 1,024-credit, 32-running queue. Cold hint-only work may hold
at most 112 maintenance items and lifetime credits and run eight tasks; known-owned
or unfinished targets use the remaining maintenance reserve and run before cold
ready lanes. Each class has its own rotating due cursor. Event and durable takeover
lanes run first without overtaking earlier work in the same wallet. Uncertain SQL
keeps its credit after its waiter ends. These are workload and memory bounds, not
measured throughput or a guarantee of renewal capacity under slow dependencies.

The wallet safety lock precedes the realm row lock; no broker, policy or media await
runs inside that SQL transaction. A fresh foreign row cannot be replaced from an
epoch comparison alone. Known exact predecessors and lapsed rows can transition;
fresh external same-session bindings are read-only. Uncertain mutations retain their
before/after row identities, assignment digest and admission work credit until a
locked conditional read settles them. No reset, predecessor restoration, broad
wallet release, assignment index or per-service pool is required.
An explicit Pulse no-selected-owner proof can retire a local retry slot only after
the same wallet-before-realm read has settled any uncertain mutation and its work
credit. The exact desired generation, selected identity and outstanding operations
must still match. Retirement performs no realm mutation or release; known settled
ownership ceases renewal under the existing lease, and foreign rows remain intact.

`social.pulse_authority_generation` holds one deployment-audience primary-key row:
positive sequence epoch, UUID source incarnation and naive UTC update timestamp.
The root Pulse process alone writes it on fresh acquisition of the existing session
lease; assignments and takeovers remain in Pulse memory. Gatekeeper reads the row
through its existing pool and takes `FOR SHARE` before wallet/realm mutations and
credential authorization. A successor commit is the authority cutover. The row does
not expire or need renewal updates, avoiding contention with selected transactions.
Separate exact-stamp, single-use mint and final-delivery permissions have an original
two-second deadline including pool acquisition; neither can be renewed from a stale
source after cutover. Already issued credentials retain their existing validity.

One bounded immutable source-specific view discovers unchanged native targets after
restart or dropped hints. At most 49,146 targets use a canonical JSON array below
8 MiB, with 256 entries per reply below 64 KiB; these are encoding limits, not measured
heap or throughput. Two readers share the same live view. A complete current view is
required before retirement of known older epochs, followed by operation-grant expiry,
exact uncertain-SQL settlement and outstanding-handle checks. Unregistered historical
sources cannot be forgotten from UUID inequality. Retirement does not write foreign
assignment rows or discard independently retained cleanup/takeover obligations.
Wallets present in the sorted complete current view retain their exact previous slots
until reseeding; absent old wallets can still retire to free bounded capacity.

This metadata is local authority, not mirrored catalogue state. Restore with writers
stopped and admissions closed, then acquire a fresh UUID/epoch before reopening.
Restoring the row is not permission to reuse a pre-restore capability. The dev trades
snapshot contains only its explicit shop tables, and content snapshots contain only
content projections. The audience primary key serves all control reads/writes; no
secondary index or assignment checkpoint is added.

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

Community and friendship producers write native notification rows through the
existing SQL pool. Notification insertion also creates the email delivery row;
the existing delivery worker owns email attempts. Community recipients follow
membership and subscription opt-outs. These producers do not publish an AWS
event or depend on an external notifications worker. Failure behavior at each
producer remains explicit: best-effort community/friendship notifications do
not undo their preceding mutation; referral grant and accepted notification
are one transaction.

Native ended-event metrics live in Social. `social.event_ended_occurrences`
records one immutable fact per `(event_id, occurrence_start)`; its RSVP count
comes from explicit local attendance, and its community must resolve to an
active, unsuspended local community. The fact and community metric contribution
commit together. Removing a catalogue event does not erase facts; deleting a community removes its aggregate. The first scan starts at
activation and does not import historical endings. Each subsequent pass reconciles
currently ended occurrences against the immutable fact keys. Duration-only edits
that move an end behind an earlier scan are reconsidered. A DTSTART or recurrence
change records a private database-clock cutoff in the authoring transaction.
Unchanged eligible starts shared by the old and new materialized schedules
remain eligible, including live occurrences and deferred endings. A private
lookup stores only those pre-edit starts still in the current schedule, bounded
by that schedule rather than growing across edits. Newly introduced backdated
starts cannot contribute. Creation and each transition into approved state start
an eligibility window using the database clock: already-ended occurrences are
excluded, while an already eligible occurrence still in progress is retained.
Thus a late first submission counts its ongoing occurrence without importing
historical attendance, and later approval cannot backfill endings from pending
review or resurrect a newly backdated schedule identity. These keys
are excluded from the public event projection and notification snapshots.
Existing immutable facts remain intact. Ordinary
owners must obtain moderation again after changing dates or durations. Recovery
is limited to the latest 30 days: older unrecorded endings require a separate
backfill, and existing immutable facts remain untouched. The approved
`(finish_at,id)` index prefilters series before date expansion; attendance and
community resolution occur after the candidate LIMIT. Each pass handles
at most 100 occurrences with transaction-local statement and lock timeouts and
an independent scan budget, so metric contention cannot stop notification
delivery. Expected workload is one periodic scan per minute; throughput at
production catalogue size remains unmeasured.

The optional event moderation webhook uses an Explore-owned transactional
outbox with public labels and moderation reasons only. Delivery is at least
once, bounded to six attempts with five-second HTTP timeouts, and starts at its
first capture cutoff, written atomically with the intent using the database clock.
Exhausted delivery attempts remain distinct from successful completion. A creator
can enqueue at most two edited notices per rolling minute; unsent edits
coalesce per event. Every submission retains its own durable alert. Approvals,
rejections and deletions precede all submissions, and submissions precede edits;
submission volume therefore cannot delay operator moderation alerts. Deployment configuration supplies the secret endpoint;
operator writes and deployment posture gate outbound delivery. Unconfigured or
operator-write-disabled authoring captures no webhook intent. The same worker
performs bounded cleanup even when its endpoint is disabled: at most 1000 rows
per pass, with a one-second client budget and transaction-local statement/lock
timeouts. Completed and exhausted rows are retained for 30 days, as is legacy
history before activation; eligible unfinished retries and the rolling-minute
edit throttle window are preserved. `(created_at,id)` supports retention order. Migration `0018` adds missing creator
and edit-bucket columns on earlier shared development layouts without removing
intents, and the object pass recreates the changed due/creator indexes.

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
All six event notice kinds use one `(address, broadcast_key)` per event and
type. A repeated notice refreshes only when `metadata.occurredAt` is strictly
newer; equal source times keep the first payload. Visibility and read state stay
as first stored, and only a new row creates a delivery. Notification rows remain
the durable deduplication evidence after intent and terminal delivery cleanup.
Runtime and the notification import hold the same transaction locks through
historical identity adoption and upsert. Each wallet maps to one of 256 buckets:
`(1163284052, (hashtextextended(address, 0) & 255)::integer)` in PostgreSQL's
two-integer advisory namespace. Distinct buckets are locked in numeric order,
bounding retained event locks at 256 even during whole-transaction dry runs. An existing keyed carrier
wins; otherwise the earliest exact native event match retains its ID. Existing
historical duplicates are not deleted. The import also recognizes exact source
IDs, retains event keys and never creates email deliveries.

History adoption performs up to 100 wallet/type/event probes per recipient page.
`notifications_event_history_idx` indexes only unkeyed rows with an eventId, in
wallet/type/event and creation order. New carriers leave the partial index;
unrelated notices without that field do not enter it. The index is a new object
installed by the schema object pass on existing databases, without a reset.

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

The Events import records the exact timestamp and payload it wrote in
`dcl_migration.event_attendance_loaded`, keyed by event and signer. Its whole-set
attendance reconciliation deletes an upstream-missing row only while it still
has that imported `going` state, timestamp and payload. Later local changes and
unrecorded local rows are retained; an empty source requires explicit approval
through the importer's `--allow-empty` option. Pre-ledger imported rows already
missing upstream require an earlier authoritative export before activation;
current rows alone do not establish their origin. The notification import needs
a full restart from retained source history to repair older missing event keys.
Missing source notice identities likewise require earlier export evidence.

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

The anonymous Registry `GET /denylist` feed reads `content.denylist` through the
existing content pool: `id`, `added_by` and `added_at`, ordered newest first.
Expected lists contain hundreds of rows; the read checks overflow at 100,001 rows
and refuses lists above 100,000. Besides console edits, one bulk write fills the
table: the one-off `catalyst content import-denylist` of the old content
server's denylist file at cutover, which writes one batched insert and one
batched `ops.admin_audit` insert under a table lock that refuses to pass 100,000
rows. The shared Explore API state retains local rows for five seconds and
coalesces concurrent refreshes. Database failures are retained for the same
interval and return 500; a failed refresh never serves an expired successful
list. Operator edits become visible on the next read after expiry, and process
restart rebuilds on demand. Source moderation responses are validated on every
request and never cached here. Merge and serialization remain per-request work;
the cache bounds table-read frequency, not total endpoint CPU or response bytes.
No table, index or pool is added. The public feed intentionally publishes every
local id kind and `added_by` as the operator's `created_by` wallet.

The public What's On calendar caches completed, parsed catalogue responses
for 30 seconds in each Sites process. Its key is the catalogue base, search,
recurrence selector and exact date window; catalogue page offsets and accounts
are not inputs. At most eight entries of up to 500 records are retained, with
expired and least-recently-used entries evicted on reads. Parsed responses
retain malformed-row counts so a damaged record does not bypass the cache. Concurrent reads of one key share one upstream request with
an independent five-second budget; each request can cancel only its own wait.
At most eight shared reads run at once; a new key at capacity fails retryably.
Failed or timed-out reads are not retained, and process restart rebuilds from
the events API. Page views reuse the same calendar read within the freshness
budget while live/catalogue reads stay uncached.

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
After the steps, the object pass creates every object of `schemas/` the
database lacks and re-creates each view it has in place, so a column a step
adds reaches the view; a view change that drops, renames or retypes a column
is a release, because the replacement refuses it.

The binary requires schema version 3. Stop all writers, pollers and administrative
write paths, and verify a backup restore before a release. A version-1 database
first takes `schemas/releases/v2-shop-schema.sql`, then a compatible version-2
binary runs `catalyst schema migrate` and `catalyst schema status`. Do not use the
version-3 binary to prepare version 2 or edit `meta.schema_steps` to bypass
preparation.

The Social voice release accepts shipped main version 2 without historical step
`0020_social_voice_notifications` or its retired objects, and genuinely prepared
version 2 with that step's matching real receipt and complete legacy layout.
`catalyst schema status` exits 5 on such a database. After verifying a backup
restore and with every writer stopped, the version-3 binary applies the release
and the additive steps in one transaction, with no statement timeout:

```sh
catalyst schema migrate --release
catalyst schema status
```

`--release` refuses while any other client session is connected to the database,
including sessions of roles whose activity the migrator cannot read, and reports
their count and application names; it changes nothing then. Server processes
without a role, such as autovacuum workers, are not sessions and do not block it.
Background workers that run as another role, such as parallel workers,
logical-replication workers and logical WAL senders, still count and block it.
It is a best-effort guard: the release locks its own tables, not every table.
A connection pooler or monitoring agent that holds sessions open also blocks it.
Exit 5 reflects the stamp only; the release's own checks (mixed layout,
receipts, destination, pending wallets) can still refuse it inside the
transaction. A database already at version 3 takes the plain migrate.
The binary embeds `schemas/releases/v3-private-voice-pending.sql` with its includes
(`db::RELEASE_SQL`). The same file still runs through psql, followed by
`catalyst schema migrate`:

```sh
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f schemas/releases/v3-private-voice-pending.sql
catalyst schema migrate
```

Keep every writer stopped through either sequence. The release refuses mixed
layouts, changed receipt hashes and a pre-existing destination. It audits pending
wallets across both roles and refuses overlapping or non-normalized pending state.
The explore parity layout (step `0029_explore_parity`) moves event moderation notices
into the shared outbox `explore.moderation_hooks`; the earlier tables
`explore.event_moderation_hooks`, `explore.event_moderation_hook_activation` and
`explore.event_upcoming_hooks` are retired by their own release. Run it once, with the
administrative role, after every replica runs a binary with the outbox and `catalyst
schema migrate` has recorded step `0029_explore_parity`; it refuses to run earlier:

```sh
psql "$DATABASE_URL" -X -f schemas/releases/explore-retired-hook-tables.sql
```

Until it runs, every pass of the `moderation-hook` job first moves the pending rows of
the old events table into the outbox, so the release finds few or none; it moves the
rest in the same transaction, keeps the earlier of the two activation instants and
drops delivered and dead-letter rows with the tables. The drop is not reversible: a
rollback restores the matching backup. Do not reset a database.

The canonical future definitions live once under `schemas/releases/v3/`; the
compatible version-3 build explicitly enrolls them in the single fresh bootstrap.
A version-2 build does not discover that nested directory. Merging dormant
release artifacts does not activate version 3 or establish cutover readiness.
In one transaction the release creates the canonical exact-key notification relation,
preserves prepared legacy generations and TTLs, verifies the backfill both ways,
retires the legacy layout when present, creates the canonical caller/callee
indexes, drops the former uniqueness constraints and stamps version 3 last. It
neither renews expired facts nor rewrites a call or participant. Legacy UUID
columns have already lost raw identifier spelling: their backfill uses `id::text`
and cannot reconstruct historical aliases.

Run the current `schema migrate` for the remaining additive layout, and verify a
clean schema status before traffic resumes. On shipped main version 2, step
`0021_social_referral_lifecycle` adds `referral_progress.id` with a volatile
default: PostgreSQL rewrites the table and builds its unique index under ACCESS
EXCLUSIVE and mints one id per existing row (see METRICS.md for a measured size).
Its other columns take constant defaults or are nullable, and step
`0023_social_referral_pending_reward` adds two nullable columns without a rewrite.
`0024_social_referral_reward_receipt` adds nullable claim, response and retry
columns, imports confirmed `reward_paid` receipts without losing notification
completion, and quarantines older uncertain pending rewards. The backfill touches
only paid or pending rewards. `0030_explore_read_columns` adds the stored read
columns of `explore.place`, `explore.place_world_local` and `explore.event` in
one `ALTER TABLE` per table, so each table is rewritten once: 0.96 s, 0.91 s and
0.14 s on 10,008, 10,008 and 3,011 rows, where one statement per column took
2.38 s, 2.07 s and 0.39 s. `0033_explore_recurrence_one_pass` replaces the
bodies of the two recurrence functions of `explore.event` in place with
one-pass bodies computing the same values: 19 ms on 7,686 rows, no row
rewritten; a write parses each `recurrent_dates` entry once per column
(5.7 µs per entry where the shipped bodies took 21 µs).

The object pass also builds `deployments_active_entity_timestamp_idx` on
`content.deployments` (the snapshot generator's inactive-entities count) in the
migrate transaction, not concurrently, so writes to that table wait for the
build: 7.8 s on 4M narrow synthetic rows (METRICS.md, "Snapshot inactive-entities
count"), unmeasured on production. Writers are stopped for a release anyway. To
build it outside that window, run beforehand, with the same definition as
`schemas/00_content.sql`:

```sql
CREATE INDEX CONCURRENTLY deployments_active_entity_timestamp_idx
    ON content.deployments USING btree (entity_timestamp) INCLUDE (entity_id)
    WHERE deleter_deployment IS NULL;
```

The object pass creates only objects the database lacks, so it then skips the
index. A failed concurrent build leaves an invalid index of that name: drop it
before retrying.

**Migration 0024 does not support a rolling deployment from a pre-0024 binary,
even when both binaries require schema version 3.** A pre-0024 binary treats any
non-null `reward_tier` as permission to POST and ignores the new claim and lock.
It could issue a reward already claimed or quarantined by the new code. Before
running `catalyst schema migrate` for 0024:

1. Pause referral/login ingress and stop every login consumer, retry worker and
   administrative writer. Drain or stop in-flight reward work. Pending native
   login observations retain their client event IDs and may retry within their
   acceptance window. Legacy SQS messages cannot be replayed into the signed
   native endpoint: they lack the user signature, and no queue importer exists.
   Any undrained legacy messages will not advance referral progress after cutover.
2. Stop every pre-0024 binary sharing this database and disable its automatic
   restart or rollback. A fleet lease or `FLEET_JOBS_ENABLED=false` alone is not
   this gate: older login consumers do not honor the new reward lock.
3. With all writers stopped and a verified backup/restore procedure, run migrate
   and schema status using the compatible binary. Inspect imported paid receipts
   and quarantined pending claims before resuming only compatible processes.

This gate applies even when the database is already version 3; the version stamp
cannot distinguish the two reward implementations. Schema migration does not
automatically enforce the process shutdown. Do not roll an old binary back onto
the migrated database; use a reviewed forward fix or a coordinated restore and
provider reconciliation, since restoring SQL cannot undo an external reward.
No production deployment history is assumed to waive this gate.

The owned upgrade test
`populated_main_v2_referrals_survive_the_upgrade_with_minted_ids` checks a
populated shipped-main table keeps every value, gains unique ids and marks its
granted rows. A database prepared at the captured version 2 also holds
`referral_progress_ip_idx`, which the bootstrap does not define and no query uses;
schema status does not report it. From version 3 onward, the exact
immutable historical 0020 is retired: apply/migrate never execute it or invent a
receipt. Status/bootstrap verify any genuine receipt and reject the five known
retired columns, constraint and index if present. A failed release transaction
preserves the version-2 stamp, facts and constraints. Both older and newer
binaries refuse a mismatched stamp.

Retained media and independent raw notification aliases can coexist in version 3.
Reinstalling the former uniqueness or single-row layout is not a general rollback.
Quiesce and use a reviewed forward fix or restore; never delete or deduplicate
media calls or notification facts to force old constraints back. Owned upgrade
tests exercise the composed release and real migrator against captured, genuinely
prepared and shipped-main version-2 state; deployment backup, restore, historical
raw-alias loss and cutover validation remain separate operating gates.

### Retired telemetry sink

The node is not a Sentry or Segment sink: the `telemetry` schema (the sink
tables, their console state and the Shop analytics rows) left `schemas/`, no
binary reads or writes it, and client telemetry goes to Segment from the sites
and the explorer. A database stamped before the removal keeps the schema until
the release script drops it; the bootstrap and the object pass neither create
nor drop it, so the old tables are inert, not an error. Rollout, in order: set
`SEGMENT_WRITE_KEY` for the sites (`track()` sends nothing without it) and
deploy the sites build without the sink's reader (`TELEMETRY_DATABASE_URL`
leaves with it); run `catalyst schema migrate` with the binary without the
sink (steps `0032_shop_sales_index` to `0035_explore_mirror_adopted_at`; its
`serve` refuses to start before them); run that binary on every replica and
confirm no `serve` of the old build is left (its ingest writer and
`telemetry-purge` job write the schema, and the database cannot tell a quiet
one apart: ingest was off by default and both builds connect alike); then
once, with the administrative role:

```sh
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f schemas/releases/telemetry-retired.sql
```

The release refuses a database without step `0035_explore_mirror_adopted_at`
or whose `telemetry.events` took a row in the last ten minutes (an old `serve`
still ingesting; the fleet check above is the gate for a quiet one), waits at
most 10 s for its locks, then drops the schema with its
twelve tables and nine indexes in one transaction and keeps `ops.admin_audit`:
its rows with domain `telemetry` and the CHECK value that admits them stay. The
deployment then drops the read-only role it created for the console's SQL
reader and the `TELEMETRY_*` settings. An old `serve` started after the drop
fails its bootstrap on the missing relations and stays down.
The drop is not reversible without the matching backup. Do not reset a
database.

Peer sync is not a backup of local state: a node taking local writes holds the only copy of that data. Before turning a write switch on, the operator backs up the database, the blob store (`STORAGE_ROOT_FOLDER`) and the credentials (`ENCRYPTION_KEY` among them) and tests a restore; the application does not enforce this. Development and commit builds sharing a database must agree on its schema; do not reset or silently migrate shared state to fit a working tree.


## Notification writer

`shared::notifications` (`services/catalyst/shared/src/notifications.rs`) is the
one writer of `social.notifications` and its `social.notification_deliveries`
email row, for every producer: Social's own and Shop's sale, royalty, transfer,
rental, on-chain bid (`bid_received`, `shop/src/market/bid_notices.rs`), tip,
favorite-discount, credits and goal notices. Writes use the producer's executor,
so the notice commits with the producer's transaction. Ordinary batches use one
statement and upsert by `id` (or by `(address, broadcast_key)` for keyed notices),
refreshing metadata and occurrence time. Worlds' three quota kinds
(`worlds_missing_resources`, `worlds_access_restricted`, `worlds_access_restored`)
carry an explicit producer persistence policy: one first notice per wallet,
kind and UTC date of block creation, including restoration on a later day.
Social sorts quota notices by wallet; for each notice, the shared writer takes
that wallet's transaction-scoped notification lock before aliasing and
inserting. The locks remain held until transaction completion, in a namespace
distinct from Worlds' quota locks. The writer links the
three exact native notice IDs derived from the block's full creation timestamp
to daily `broadcast_key` values where no daily carrier already exists before
inserting the current notice. A linked row keeps its ID, metadata, occurrence
time, read state and email delivery records; a new daily notice uses the
canonical daily ID. Repeated daily keys leave that first row unchanged.
Aliasing and insertion use the mutation's
transaction and existing notification keys; no table or index is added.
Only a new notification gets a `pending` delivery row, or `skipped` with the
reason when the type has no template or the recipient cannot receive it.
Social owns both tables, reads them and delivers the email;
`with_emailable_kinds!` is the one list of templated types the writer and
Social's templates share.

The [quota notice identity decision](docs/adr/2026-11-21-world-quota-notice-identity.md)
retains a cutover boundary: already closed native blocks or imported rows that
cannot be linked from exact creation facts can overlap with the first daily
notice. This writer does not reconstruct missing timestamps or consolidate all
historical duplicates.

## Social realtime notification workload

Social keeps each notification scope, generation UUID and 24-hour expiry in
`social.community_voice_notifications`. Its case-sensitive `raw_community_id`
text primary key preserves the source cache key, while `community_id` is the
canonical foreign key used for membership and media. Several spellings of the
same UUID may retain independent facts. Non-null fields and identity/scope checks
couple their lifecycle. START upserts one exact key with a new generation; a
45-second default poll reads raw keys in ordered PK pages of 64. It refreshes an
active key's expiry, or clears an inactive/expired key, by exact key plus generation.
A media closure does not identify a raw key; explicit END handles only its supplied
spelling, and other aliases await polling. No SQL lock spans a media or NATS await.
Generation CAS protects metadata from stale cleanup, without making external
media teardown atomic with a competing START. The raw primary key supports all
runtime reads/writes; expiry is checked in the bounded pages, so it needs no
separate expiry index.

`social.event_payloads` is an optional immutable transport cache for community
voice metadata whose event exceeds the existing 64 KiB NATS admission limit.
The writer inserts opaque binary protobuf bytes under a random UUID, capped at
512 MiB and expiring after 24 hours. Normal events require no insert. Receivers
perform one fresh-only PK lookup for an oversized event; separate snapshots cannot
overwrite each other. Private-call credentials never use this relation. The
`event_payloads_expiry_idx(expires_at,id)` supports ordered deletion of at most 64
expired rows with `FOR UPDATE SKIP LOCKED` per polling pass. This relation does not
record delivery, retry, or replay work.

The expected hot set is tracked active raw voice keys plus oversized events
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
ranking job updates scores for local authoritative communities and preserves their
`updated_at` timestamps for import conflict resolution. Mirrored ranking scores
remain upstream facts. `ranking_tracking_since` bounds local event occurrences
eligible for collection; zero counter defaults do not reconstruct history. The
social-service-ea import copies the lifetime counters and moves
`ranking_tracking_since` to the moment it read them, so the collector counts only
occurrences that end later; it imports no producer receipts. See "Community ranking"
in `tooling/migration/sources/social-service-ea/MAPPING.md`.

The existing leased event collector keeps a scalar source-key cursor and a
recurrence ordinal window. It selects 100 event keys through the existing primary
key before reading schedules, expanding at most 100 recurrence positions per
event or probing receipts. One pass shares a three-second budget across at most
1,024 pages and commits at most 100 new occurrences. Page receipts and lifetime
counter changes commit before cursor advancement. Cancellation, SQL failure and
process restart retain durable deduplication; cursor state is only local work
progress. Full cyclic revisits reconsider backdated inserts, duration edits and
appended recurrences. No end-time watermark hides these changes. A long catalogue
or recurrence list can take multiple minute passes; these bounds do not establish
ranking freshness in production.

`social.community_ranking_events` retains local producer receipts with primary key
`(kind,event_key,community_id)`. One photo may contribute to several communities;
receipt insertion and counter changes share a transaction. Receipts have no expiry.
External decoded events preserve the upstream consumer's repeated increments,
including its finally-acknowledge behavior on handler failure. The receipt relation
does not turn that transport into an exactly-once consumer.

Community deletion probes `community_voice_notifications.community_id` and
`community_ranking_events.community_id` through their cascading foreign keys.
Both have a leading community-id index. The raw notification key and the ranking
receipt's `(kind,event_key,community_id)` key serve different access patterns.
Synthetic warm measurements and insertion costs are recorded in `METRICS.md`;
they are not production capacity measurements.

Camera uploads retain their multipart basename in `camera_reel.images.original_filename`.
Gallery URLs concatenate the configured API base, row UUID and basename literally;
thumbnail names add `thumbnail-`. Reserved filename characters remain raw in JSON
and redirect Location values, as in the pinned source. To request a complete
filename containing `?`, `#` or `%`, clients must percent-encode its path segment.
Alias downloads extract the UUID and use the existing primary key to resolve the
shared image or thumbnail hash. The nullable column adds no index or backfill:
existing rows retain their hash URLs. Migration `0045_camera_reel_original_filename`
adds the column without replacing rows or blob bytes. Local `/api/files` downloads
apply the camera row's review state and the shared blob denylist. Image redirects
preserve the requested filename without querying the database. After storing both
blobs, a new upload releases previous deletion withholding for those hashes only
when a live non-rejected photo references them and no rejected photo does. It
retains moderation rejections and operator or remote denials. Configure
`CAMERA_REEL_BUCKET_URL` as the public API base plus `/api/files` to use this virtual
origin, or route the existing image origin to that handler. A hash-only origin
needs an ingress/configuration adjustment before cutover; it cannot serve the
filename aliases directly. An external public origin is responsible for restricting
its own copies. The local byte route serves complete GET responses, without Range
or ETag support; HEAD returns 404. It does not implement the S3 object API.

Camera upload MIME is retained per photo in nullable
`camera_reel.images.image_content_type`, constrained to the source's accepted
`image/png` and `image/jpeg` values. Both object aliases return that claimed MIME,
including when their detected encoding differs. An upload adds this short string
to its existing row insert; alias downloads select it with the existing UUID
primary-key lookup. The column adds no index, table, pool or extra read query.
Migration `0044_camera_reel_image_content_type` preserves existing rows without
inventing an object MIME backfill. Legacy NULL values and raw hash URLs retain byte-format sniffing. An import
can populate the per-photo value from object metadata; a raw hash alone does not
identify which photo's claimed MIME to use.

New camera uploads start with `ranking_published=false`. After both blobs are
written, one transaction marks the row and applies local photo contributions.
Existing photos receive `true` during migration, preserving the history boundary.
The existing leased minute job keeps two scalar UUID cursors over the partial
`images_ranking_pending_idx`. Each query fetches 100 pending keys before age
classification. Recent and cold sweeps alternate while due, with a frozen
ten-minute age boundary per sweep. A pass visits at most 32 pages under one
three-second budget, including storage and SQL. Cold retries wait an hour after
the full cold sweep; recent sweeps wait a minute. These are bounded candidate
policies, not a production freshness guarantee: a large backlog adds whole-sweep
latency, as measured in `METRICS.md`.

Both blobs must exist before the publication transaction. A candidate one-second
row budget covers both existence checks and publication; repair transactions set
a local SQL statement timeout from the remaining row budget. The direct upload
finalizer keeps its existing timeout behavior. A row error or row-budget timeout
logs and advances that pending row so later rows can proceed and a later sweep
can retry it. Job cancellation or the shared pass deadline retains the in-flight
row and unvisited tail. Restart resets only the two local cursors and delays. Durable markers and receipts prevent repeat
contributions. Missing blobs retain their pending rows for later completion;
nothing is expired or marked published merely to shrink the backlog. Deletion
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

The compliance adapter retains one lazy Outbound provider profile per community
State. Its 600-second SDK budget ends at response headers; a progressing body may
continue beyond that budget. The native connect and read inactivity budgets are
10 and 300 seconds. Provider bodies have no separate 4 MiB cap. Source-paired
factory/SDK and native-fetch probes cover these boundaries, missing versus empty
credentials and compliance metric outcomes. Delayed DNS, combined network phases
and deployed providers remain unverified; the probes do not establish exhaustive
provider equivalence.

## Community place ids

`social.community_places.place_id` holds a places catalogue id or a world name,
lowercased on every write (REST add, community create and edit, federated add)
because explore keys both lowercased. Until the release below runs, a row written
before this rule still answers: an add skips a place held in another case, removal
and the edit's held-place diff match `lower(place_id)`, and an edit lowercases the
places it keeps in place (adder and time kept) in the same transaction as its
removals and inserts.

The release script `schemas/releases/community-place-ids-lowercase.sql` collapses
case variants of one id in a community to one row (the lowercase spelling, else
the earliest added) and lowercases the survivor. It also adds the check
`community_places_place_id_lowercase` (`place_id = lower(place_id)`, declared in
`schemas/40_social.sql` for a new database) as `NOT VALID` when missing and
validates it after the merge, so no later writer stores another case. Run it after the updated
binaries replace every old replica: an old replica's mixed-case write fails once
the check exists. It needs no maintenance window, sets a 10 s `lock_timeout`,
holds an exclusive lock on the table while its first run adds the check, and is
rerunnable. Rollback needs the matching backup.

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
transaction. Each complete Slack effect has a ten-second absolute deadline, at
most three attempts and a two-second Retry-After cap, including shared provider
permit and pause waits. A failure retains the committed state and is swallowed
at the existing boundary. Same-referrer retries exit before alert delivery.
These timing limits deliberately differ from the pinned source SDK defaults.

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

Wallet-scoped media probes use `private_voice_chat_participants_address_idx`
and `community_voice_chat_users_address_idx`, both nonunique B-trees on
`address`. These are separate from the pending-call party indexes. They retain
every status so expiry predicates can use their existing timestamps without
time-dependent partial indexes. See the [wallet probe measurements](METRICS.md#wallet-presence-and-world-quota-lookups).

The object pass builds missing wallet indexes in the migration transaction,
blocking writes to the indexed tables until the transaction commits. To build
ahead, run each definition from `schemas/40_social.sql` with `CREATE INDEX
CONCURRENTLY` outside a transaction. Migration, bootstrap and schema status
check the index name only. Before migrating, verify `pg_index.indisvalid` and
compare `pg_get_indexdef(indexrelid)` with the schema. Drop and rebuild an
invalid or mismatched index before relying on it.

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

Member-request summaries read the pending request page and its total before any
community lookup. An empty raw page returns immediately. A nonempty page adds
one community-summary query bounded to its community IDs, followed by the
existing relationship read even if every community is inactive or bans the
caller. Voice and profile enrichment operate on surviving summaries. Results
follow the saved raw request order, and total/page metadata still describe all
pending requests; visibility filtering can leave a page empty. These statements
use the same pool without a transaction or a repeated read of request rows.

Signed V1 member and ban rows retain `lastFriendshipAction` and `actingUser`
from the existing latest-action query, including SQL null when no action exists.
Their numeric status uses those same facts; this adds no SQL statement, table,
index or pool. Anonymous members omit the aliases and relationship read, and V2
returns only the numeric status. The pinned source's complete response bodies
are compared for accepted, sent/received requests, absent friendship and empty
history, with V2 and anonymous controls; no performance change was measured.

## Message router workload

The community chat message router (`catalyst-message-router`, its own process with its own
pool, `MESSAGE_ROUTER_DB_MAX_CONNECTIONS`, default 4, 5 s statement timeout) reads social's
community tables by shared SQL and writes nothing. Per routed packet (a community chat message
or reaction, at most 10 per sender in a burst and 5 per second sustained) it runs one statement
with two parts:

- membership: `EXISTS` over `social.community_members` on `(community_id, member_address)`,
  one probe of `community_members_pkey`, whatever the community's state;
- recipients: `ARRAY(...)` of `social.communities` by `id` (its primary key), `active` and not
  `suspended`, joined to the community's `social.community_members` rows through the
  `community_members_pkey` prefix, every member but the sender, `lower(member_address)`. The
  result is the community's member count, which the router publishes in batches of 100
  identities.

The router routes at most as many packets at once as the pool has connections, so a packet
never waits for a connection; one that finds every routing slot taken is dropped and counted
`overloaded` instead of timing out on the pool.

Inputs are a community uuid from the packet topic and the sender's LiveKit identity, lowercased
(member addresses are stored lowercase). No table, index or column is added. Changing a column
either query reads updates this section and the root `AGENTS.md` comms ownership in the same
change.

## Native referral login workload

`POST /social-api/v1/client-login` derives the invited wallet from ADR-44 signed
fetch. Its signed metadata contains `loginId` and `occurredAt`; the timestamp
limits admission to the preceding 24 hours with five minutes of future tolerance.
The request itself still passes the 60-second signed-fetch window.
The client preserves these values across retries and signs each request afresh.
The occurrence timestamp bounds replay acceptance, while login-day accounting
continues to use the database's UTC processing date.

Most logins have no referral attribution and require only the indexed invited
wallet lookup. For an attributed wallet, `social.referral_login_receipts` stores
one receipt keyed by `(invited_user, login_id)` in the same transaction as the
login-day update or grant and its accepted notification/email delivery row.
Duplicate receipts do not evaluate the threshold again. The FK to referral
progress shares its deletion/restore lifecycle and cascades on deletion.

Under the invited-user row lock, admission caps distinct new IDs at 120 per
wallet in a rolling 24-hour window of server `received_at` values. A duplicate
does not consume another slot. Finished or rejected referrals return success
without adding receipts; banned referrers are a no-op on this native ingress,
without exposing their identity or stored IP. Those acknowledged login
observations are not retained or replayed if the ban is later lifted; a new
login observation is required. The referrer-wide advisory lock
is acquired only when the login can grant a tier, after receipt checks and
login-day processing. Lock or statement timeouts return a retryable 503.

Expiry is the later of server reception and signed occurrence, plus 24 hours.
Backdating cannot shorten quota retention. Future-dated events can retain a
receipt for up to 24 hours and five minutes, so retained receipts are not the
same count as the rolling admission limit. The primary key's invited-user
prefix serves this bounded per-wallet count; no separate receipt-time index
is needed. The existing minute worker locks and deletes at most 1,000 expired
receipts with `FOR UPDATE SKIP LOCKED`, using
`referral_login_receipts_expiry_idx(expires_at, invited_user, login_id)`.
Cleanup failure is logged and does not skip the reward pass. Delayed cleanup
does not extend accepted event age or the admission window. These are
implementation bounds, not measured production rates or throughput.

The bootstrap defines this new relation and index. Run `catalyst schema
migrate` before rolling out the new server. On an existing database, the
production migrator creates missing objects from that same definition. Step
`0025_social_referral_login_received_at` also upgrades databases that already
have the earlier receipt table without `received_at`, preserving its rows,
keys and original expiry timestamps. Each backfilled timestamp uses the UTC
server clock after the DDL lock is acquired; future inserts use the same
server-clock default. Existing `received_at` values remain unchanged. The step
is recorded once and a second migrate does not change the timestamps. The
volatile default can rewrite the table under the DDL lock; no production-sized
migration duration has been measured.

Both receipt cleanup paths require `expires_at` to have passed and
`received_at` to be at least 24 hours old. This preserves the backfilled quota
window even when a legacy expiry is already in the past. The effective
retention for such upgraded rows is at least 24 hours from each row’s
backfilled timestamp. The
workload remains the existing per-wallet receipts; no new table or index is
added, and no receipt or referral progress is reset.

Referral attribution serializes quota decisions with a transaction-scoped
referrer/IP advisory key; count and insertion share that transaction. The IP is
the actual TCP peer unless an explicitly trusted backend proxy supplies one
valid overwritten X-Real-IP. New facts use canonical addresses; the count uses
guarded inet equality over historical text, including mapped IPv4 aliases.
Malformed historical values remain stored and do not abort the count. The
existing referrer-leading index bounds the prefix; alias conversion is still
per matching row and has a measured CPU cost (see METRICS.md). No index on the
stored IP text serves this inet comparison, so the schema defines none. Finalizing
a referral serializes its referrer and counts the milestone before committing
the grant. A grant that reaches a reward tier records it in the nullable
`referral_progress.reward_tier` in the same transaction. Optional milestone Slack alerts occur after commit. At most eight such tasks
per adapter are admitted before spawning; saturation drops and logs the alert.
Admitted effects retain the ten-second deadline. Reward requests run in the existing
leased worker after login recording commits; they do not delay login acknowledgement. One nullable receipt per milestone stores a
pre-POST `reward_requested_at`, confirmed provider `reward_response` and
`reward_retry_at`. The expected workload is at most one receipt per milestone,
with a 64-row due batch every minute; a tier with an empty supply is asked once
an hour, plus once per new grant; this bound is not a throughput measurement.
The partial due index excludes completed and ambiguous requests. A session
advisory guard serializes attempts without an open transaction across HTTP; its
connection closes on cancellation.

Configuration failures leave the request unclaimed. After the POST claim commits,
transport errors, non-success statuses, malformed replies and a crash before
response persistence leave an ambiguous receipt that is never automatically
reissued. Operators must reconcile the outcome before restoring it to pending or
supplying a confirmed response. A held claim with no stored response stays
quarantined until an operator clears `reward_requested_at`. A successful reply
`{"ok": true, "data": []}` means the tier's campaign has no stock left ([greeter-dapp
reads it as "No Stock"](https://github.com/decentraland/greeter-dapp/blob/30631bd10335f6f89f708829c26c3d679f9a626f/src/api/Rewards.ts)): the claim is cleared and
the tier's other unclaimed rows without a stored response move one hour ahead
in their current order, through the partial due index, with the answered row
placed last. A reply for a new grant moves only rows that are due or new, so a
row already waiting keeps its time; a reply for a waiting row also moves every
row due within the hour. One request per hour therefore covers the tier, each
asking the next row, plus one per new grant on arrival, until its campaign is
restocked. After restocking, operators may clear
`reward_retry_at` on those rows to retry sooner. Additive step `0024_social_referral_reward_receipt`
imports previously confirmed object-valued `reward_paid` responses, including
receipts with missing or invalid image metadata, and quarantines uncertain pending
markers. A confirmed receipt with unusable image metadata remains confirmed and
needs metadata repair for image delivery; it must not authorize another POST. This avoids automatic duplicate issuance but cannot guarantee
exactly-once external delivery.

`reward_paid` retains the historical `{tier, reward, notified}` receipt while
outstanding effects remain. A confirmed response permits the worker to retry
local completion without issuing another reward. Image persistence, the native
notification and email delivery row, and clearing `reward_tier`/`reward_paid`
share one transaction. A SQL failure rolls back those effects together. An
imported receipt already marked notified does not send another notification.
Images remain unique per referrer and tier. External reward issuance still
cannot be made exactly once without a provider idempotency contract.

Community membership mutations use one PostgreSQL session advisory guard per
community on the shared pool. BAN, direct join, request acceptance, bulk public
acceptance, the SQL phase of ownership transfer and every federation role action
(join, leave, role, ban, unban) serialize under that guard. A federation action
that grants membership re-checks the active ban inside its guarded transaction,
so a ban committed after its pre-check refuses it.
BAN retains separate committed kick, unlike, analytics, ban and request-removal
phases. Ownership NAME validation precedes its guard; external notification,
voice and provider awaits follow release. SQLx 0.9 native acquire already owns
its cancellation cleanup. Acquisition has one absolute one-second budget
covering pool checkout, temporary BEGIN, SET LOCAL, native acquire and COMMIT.
The local server lock timeout is 750ms; the connection is marked close-on-drop
until COMMIT is confirmed, then disarmed synchronously. This bounds waiting
backend retention while preserving the same connection, autocommit and previous
GUC values after success, including a pool of size one. Normal release remains
native guard Drop. These limits strengthen coordination and bound contention;
the separate source commit phases remain independently committed.

Private media credentials serialize same-call writers before a plain party read,
then take sorted wallet transaction locks before locking/upserting the call.
A conflict preserves its existing parties, creation time and prior participants.
Pending expiry remains effective while a media writer waits for a wallet.

### Durable world admission edits and stable bans

World scene bans use `<world>:<base parcel>` keys; Genesis bans use the place
("Scene bans" below). The release script `schemas/releases/world-scene-ban-keys.sql` consolidates active and retained
world scene keys, keeping the newest ban when aliases overlap. It removes mapped
legacy aliases so an unban cannot leave a hidden effective ban. Unmapped rows are
retained. The existing retired-scene table supplies mappings during its configured
retention period; a live world scene whose deployment is positively absent is
removed during maintenance. Authority errors remain undecided.

Access edits commit a unique revision and database-clock cutoff to
`social.world_access_rechecks` in the same transaction. Community closures persist
retry intent before their advisory wakeups. Maintenance shares one room inventory
between 16 due edits and a rotating batch of 16 live worlds. Cutoffs stay after
success so an old token cannot regain access by reconnecting. World tokens carry
additive signed authorization attributes bound to world, wallet, room and mint
start time. The short `FOR SHARE` stamp orders against existing access writers.
The `dcl.worldAuthorization` participant attribute survives normal client metadata
replacement. Previously issued signed metadata proofs remain readable when the
attribute is absent; an invalid present attribute fails closed. Legacy unsigned
tokens and key rotation may require a fresh mint after an access edit. Existing main ownership, ENS and scoped grant policy
still decides admission.

The caller secret budget keeps the existing `attempts/window_start` table and
configured limit. Mint, adapter and scene-host paths share it. Attempts charge
before verification; cancellation keeps the charge until expiry. Non-invalid-secret
answers refund only their exact charged window. Expiry cleanup deletes in batches
of 1,000 rows and continues while a batch is full. The 10-second sweep budget is
checked between batches, so an in-flight batch can overrun it; it is not a hard
deadline. This does not add a cross-caller world ceiling.

For rollout, stop all old API and scene-host replicas before running `catalyst schema migrate` and then applying
`schemas/releases/world-scene-ban-keys.sql` with the administrative database role, then start the updated binaries. `catalyst serve` refuses startup while active or retained
world scenes still have mapped entity-id bans awaiting this conversion; fresh databases and
unmapped bans do not require it, and Genesis bans have their own release ("Scene bans"). Old binaries cannot enforce the
new stable ban keys. A binary-only rollback is not supported after rekeying;
restore the matching pre-migration database backup. Do not reset a database.

### World admission SQL and completed access checks

World admission stamps and shared-secret refunds each use one versioned SQL
function. The function names include their signature, settings and body hash.
The schema object pass installs the functions on existing databases. Run
`catalyst schema migrate` before starting the new binary; `catalyst schema functions`
also prints their administrative installation SQL. Existing function versions remain
available to old binaries during a rollout. Admission returns unavailable if its
required functions are missing.

The stamp retains `FOR SHARE`. It must wait for non-key access updates from older
binaries as well as newer access writers. A function-local 500 ms lock timeout
restores the caller's setting after success or failure. A one-second client deadline
bounds the stamp and refund wait, including pool acquisition. The server retains
the pool's configured statement timeout; a client timeout does not prove that a
dispatched server statement stopped. Stamp timeouts return unavailable, and a
refund timeout leaves the reservation to expire unless its dispatched refund completes.
Refunds lock and modify
only the exact caller window charged by the reservation; the positive-attempt
constraint and the existing table layout remain unchanged.

A completed world access check retains its revision and authorization cutoff, but
sets `next_attempt_at` to PostgreSQL infinity. Completion compares both the revision
and the exact claimed schedule, and checks that the lease is still unexpired after
acquiring the row lock. A newer edit, later claim or same-revision community wakeup
cannot be consumed by an older pass. Completion-write failures are logged and leave
retry intent intact; they do not stop the remaining batch or the live-world sweep.
Community closure advances the schedule with `LEAST`, including completed rows. The live-world sweep still reads retained
cutoffs. After completion, stale-token re-enforcement relies on that rotating
sweep rather than repeated due-queue work: visiting all live worlds takes
`ceil(live_worlds / 16)` maintenance ticks (approximately that many minutes at
the normal 60-second cadence). Outages and slow calls can extend the wall-clock
latency; this is not a guaranteed deadline. The application decodes only finite
claimed schedules and never decodes infinity as a Rust timestamp. This change adds
no migration column and does not relax the stable-ban migration's coordinated
rollout requirements.

## Segment webhook events workload

`POST /events-notifier/forward`, `/forward-web` and
`/forward-onboarding-checkpoint` are the Segment destination webhooks the
events-notifier served. The first two store the event they accept as one row of
`social.client_events`, inside the request, after the signature, the context
checks and (client events) the auth chain verified. Writers are those two
handlers at the destination's delivery rate: the explorer reports a
`move_to_parcel` per parcel crossing and the distance and height events on
thresholds, so the expected order is one row per active player per minute; no
production rate is measured. Readers are consumers scanning forward in
insertion order (`seq`) from a cursor of their own (the badge processor, below)
and the purge;
the trigger `client_events_notify_trigger` announces each stored row's identity
columns (never the metadata) on the `social.client_events` channel for
listeners outside the request path.
That channel is a data-exposure surface: Postgres grants `LISTEN` to every
role that can connect, so each such role receives the wallet, the IP, the
anonymous and session ids, the realm and the event names of every stored
event; the database's roles are the node's own and the flagging service's, and
a role granted to anyone else reads this feed. The identity columns are
bounded at 512 bytes each and free of control characters by the parsers (a
longer or control-bearing optional value is dropped, such a `messageId`
refuses the event), so the escaped payload stays under `pg_notify`'s 8000
bytes and the key under the index's limit.
A row is about 0.3-0.6 KB: `metadata` holds the sub-type part only (parcel,
emote, passport, distance and step count, height, runtime; the web identify's
agent, page and locale). The auth chain is verified at the ingress and not
stored: no consumer reads it.

Invariants: `key` is Segment's `messageId`, so an at-least-once redelivery
inserts nothing (`INSERT ... SELECT ... WHERE NOT EXISTS` on the key, then
`ON CONFLICT (key) DO NOTHING` for a concurrent duplicate; the check comes
first so a redelivery draws no `seq`); `address` is the
owner of the auth chain the event carried (client: the chain proves that the
wallet once delegated to a key, not that it sent this event, so a consumer
reads the row as a claim about the wallet, not by it, and never flags a
wallet on `source = 'client'` rows alone) or `traits.ethAddress`
(web), lower-cased and checked by the table; rows are immutable; `reported_at` and
`received_at` are the client's and Segment's clocks and `created_at` the
node's. Nothing references an event and an event references nothing: no FK.
Deletion is the `client-events-purge` job only: hourly, leased, it deletes rows
whose `created_at` is older than `CLIENT_EVENTS_RETENTION_DAYS` (default 30)
in batches of 5,000 keys through `client_events_created_idx (created_at,
key)`, at most 200 batches a pass (a million rows an hour, some 280 inserts a
second sustained). The badge processor's cursor stays above the purge horizon
while it runs: it passes rows seconds after their insert, so the purge only
reaches rows a stopped processor never read (their badges are then not
counted, which is the accepted cost of the retention).

The bootstrap (`schemas/40_social.sql`) defines the table, the indexes and the
trigger; an existing database gains them from the object pass. Step
`0038_social_badge_processor` adds `seq` to a table that predates it as a
nullable column, then its default (`nextval` of `social.client_events_seq`,
named as text so the step precedes the object pass that creates the
sequence): both change the catalog only, no row is rewritten, and the kept
rows keep a NULL `seq` (they precede every cursor, which starts at the head).
Both take ACCESS EXCLUSIVE on the table, and `migrate` runs the steps and the
object pass in one transaction, so the lock is held until it commits: reads,
the webhook inserts and the purge all wait for the rest of the migration.
The object pass then builds `client_events_seq_idx` (`seq`) and
`client_events_session_moves_idx` (`address, session_id, seq` over
`move-to-parcel` rows), both partial on `seq IS NOT NULL`, in that
transaction, one heap scan each with nothing to sort. On 2M
synthetic rows (822 MB, about two months at the expected rate) the step took
22 ms and the two builds 0.24 s and 0.35 s with a warm cache; the identity
column it replaces rewrote the same table in 27.7 s under ACCESS EXCLUSIVE.
To take even that off the migrate window, add the column by hand first
(`ALTER TABLE social.client_events ADD COLUMN IF NOT EXISTS seq bigint`, the
step's first statement, without the default: the sequence does not exist
yet), then build both indexes with the definitions of `schemas/40_social.sql`
and `CREATE INDEX CONCURRENTLY`; `migrate` then sets the default, creates the
sequence and skips the indexes (a failed concurrent build leaves an invalid
index: drop it before retrying).
`client_events_seq_idx` serves the cursor scan and
`client_events_session_moves_idx` traveler's read-back of one session.

## Onboarding checkpoints workload

`POST /onboarding/checkpoint` and `GET /onboarding/pending-nudges?sequence=1..3`
(`services/catalyst/explore/src/api/onboarding.rs`, bearer `ONBOARDING_API_KEY`)
keep the auth server's onboarding component (`decentraland/auth-server`
`src/ports/onboarding/component.ts`, at 08461d2b) in
`explore_api.onboarding_checkpoints` (`schemas/11_explore_api.sql`) over the
explore pool, so every `apis` replica the gateway picks reads what any other
wrote and a restart loses nothing. Writers: the auth server's checkpoint posts
(1-7 per onboarding user) and the Segment `/forward` webhook's CP7 for a
verified wallet (`social::segment_webhooks`), each a request. Reader: the
mailer's poll of the nudge list. No production rate is measured; the expected
order is one row per onboarding user and checkpoint reached, at most 7 per
user, about 150 bytes plus the metadata (at most `MAX_METADATA_BYTES`, 4 KiB,
as compact JSON; checked by the handler).

Invariants: one row per `(user_id, checkpoint)` (the unique key);
`reached_at` is the first `reached` and never moves; `completed_at` is set
once, by `completed` or by reaching the next checkpoint; email, wallet and
metadata are only filled in (`COALESCE`), never cleared; `wallet` is stored
lower-cased (checked). A checkpoint is one transaction: the wallet's
resolution (a `wallet`-identified checkpoint without an email takes the user
and email of the lowest checkpoint, then the oldest row, bound to that wallet
with an email), the `INSERT ... ON CONFLICT (user_id, checkpoint) DO UPDATE`
and the completion of the previous checkpoint; concurrent reports of one
checkpoint from several replicas meet on the key. A wallet-identified
`reached` that resolves to no email inserts no row (no nudge could reach it),
as before. `id` is the insertion order: the nudge list's order and the
resolution's tie-break. Nothing references a row: no FK.

The nudge read selects each user's highest checkpoint that is open, has an
email and was reached before `now - 12/24/36 h`: the candidates through
`onboarding_checkpoints_pending_idx` (`reached_at`, partial on `email IS NOT
NULL AND completed_at IS NULL`, the open rows only), the later-checkpoint
probe through the unique key, sorted by `id`. The resolution reads
`onboarding_checkpoints_wallet_idx` (`wallet, checkpoint, id`, partial on a
wallet and an email), one row. The upserts and completions go by the unique
key. A test checks both reads' plans with sequential scans priced out.

Retention replaces the in-process cap (10,000 rows, the oldest evicted
first), which only bounded memory: a row is deleted `RETENTION_DAYS` (30)
after it was first reached. The nudges fall due at most 36 h after the latest
checkpoint, so a row that old serves only a late wallet resolution, and
without a bound the open rows of users who stopped would make the nudge
list grow forever (the node records no sent nudge, below). Every `reached`
write deletes, after its commit, those of the 16 oldest rows by `id` that are
past the retention (`FOR UPDATE SKIP LOCKED`, so two replicas never wait on
each other); `reached_at` follows `id`, so expired rows are at the head of
the primary key and the sweep reads 16 index entries. Sixteen per insert
outruns the rate at which rows expire unless the inserts fall by more than
16 times within a retention; a failed sweep is logged and the next write
retries. Clients see the same contract: rows unseen for a month leave as the
oldest rows did under the cap, with no fixed row count behind it.

Differences from the auth server, kept from the in-process store: no
`email_nudges` table (the node runs no nudge mailer, so no nudge is marked
sent and the list holds every due user until they move on, complete or
expire); `id_type` and `source` are validated and not stored (nothing reads
them); a wallet-identified checkpoint binds the identifier as the wallet even
when the body carries an email; ties in the resolution go to the oldest row.

The table and its indexes are new objects: the object pass of `catalyst
schema migrate` creates them on a stamped database (a step under
`schemas/migrations/` may not create objects), with no rewrite of any
existing table. Checkpoints the in-process store held before the upgrade are
not carried over.

## Credits wallet flags workload

`credits.wallet_flags` (`schemas/60_ops.sql`) is the multi-accounting flag
notified on the `credits.wallet_flags` channel, `{action: "flag", addresses,
source, ip}` (`shop/src/credits/wallet_flags.rs`), or posted through the
admin, `POST /credits/admin/flags {addresses, source, ip}` (the shape the
events-notifier posted to the credits server's `/flag`): one row per flagged
wallet, keyed by address, with the source that raised it, the IP it reported
and the wallets it was linked with (`linked_addresses`, the body's list). The
node runs no detector: it keeps the flags it is told of.

Writers are the leased `credits-wallet-flags` listener (where
`OPS_WRITES_ENABLED` is on) and the admin's `POST /credits/admin/flags`, the
same upsert per address with one `ops.admin_audit` row per wallet in the same
transaction (the listener's actor is `pg:credits.wallet_flags`), the notified
or admin unflag (`DELETE /credits/admin/flags/{address}`) and the credits
operator's unblock
(`admin_store.rs` `admin_set_blocked`), which deletes the wallet's flag in the
transaction that clears its block. Readers are the credits claim (`EXISTS` by
address, folded into the captcha claim and the balance reports as
`is_blocked_for_claiming`; the spend and the reservation read the operator's
block alone, so a flag never holds credits a wallet has) and the admin list
(`GET /credits/admin/flags`, by address or paged by `flagged_at`). Rows are
small and few: a flag per wallet reported.

The channel's writers are whoever can connect: Postgres grants no privilege on
`NOTIFY`, so every role of the database can raise or release a flag, and the
listener applies it as an audited admin action. The roles are the node's own
and the flagging service's; a role granted to anyone else can flag wallets, and
a flag raised while no listener is connected is lost. A queue table under `GRANT
INSERT` to one role, or a `SECURITY DEFINER` function, would name the writer
and keep the flags raised while no listener is up: the next step if the
database gains other roles.

Invariants: a flag is a signal, not a verdict: it holds the credits claim
alone, and the admin's unflag or the operator's unblock deletes it; nothing
else does, and a flag never expires. A wallet has at most one flag (the
primary key); flagging it again keeps `flagged_at` and takes the new source,
IP and links. Addresses are lower-cased and checked by the table; the handler
normalises the body's. The bootstrap defines the table; an existing database
gains it from the object pass, so no step exists.

## Credits checkout and broker claim edges

A credits checkout line buys through the shop broker under the claim key
`checkout:{checkout_id}:{outbox_id}` in `shop.broker_purchases`. Two SQL edges
cross the areas. The broker claim (`shop/src/checkout/claims.rs`,
`claim_broker_purchase`) reads `credits.checkouts` by the id in such a key,
`FOR SHARE`, and neither inserts nor re-arms the claim while that checkout is
`reversing`, `reversed` or `failed`; `checkout_frozen` reads the same predicate
to answer a refused request. Credits writes `shop.broker_purchases` through
`claims::fence_unrecorded`, called by compensation (`credits/fulfillment.rs`)
and, in its closing transaction, by the manual refund
(`credits/checkout.rs`): it marks the frozen checkout's unconfirmed lines'
`pending` write-ahead claims with no `tx_hash` as `error`.

Invariants: once a checkout is frozen, no line of it gains a recorded buy. The
freeze is a row update of the checkout, which conflicts with the claim's
`FOR SHARE` read: a claim that commits first is fenced, one that waits reads
the frozen status and is refused, and the claimant's record (fenced on
`pending` and its claim time) matches no fenced row. The fence covers only
claims with no transaction recorded: a buy already recorded (`sent`, `bought`,
`forwarding`) still settles and may deliver. Compensation holds such a line
out of its refund; the manual refund pays the amount the operator gives. The
manual refund locks the wallet, then the checkout (`FOR UPDATE`, refusing
unless it is still `fulfilling` or `fulfilled`, so it never refunds a checkout
compensation is reversing), then the claims; a transaction that locks a
checkout and a claim takes the checkout first, so the edges add no lock
cycle. Changing the
key format, the frozen statuses or either statement updates this section.

## Daily MANA/USD closes

Shop stores one row per UTC day from July 8, 2021 in `shop.mana_usd_daily`.
The writer selects the last round at or before the end of the day from the
Polygon MANA/USD oracle using one finalized block hash per pass. Each row keeps
the phase-aware round ID and the exact decimal rate. A day without a positive
close stores a null rate. The mainnet sales index process runs a leased job
that reads at most fourteen consecutive days per pass and commits each
completed day. A failed read resumes at the first unrecorded day. Once caught
up, the job adds one row a day.

The rate endpoint scans the date key over at most fifteen years (approximately
5,500 rows). Seller summaries join the days of the requested sales by that
same key. They sum gross MANA at the historical closes, round to six decimal
places and count sales with no rate. Royalty totals use the settlement legs
recorded for resales of the creator's items. The sales publication and all
inputs to these calculations are read in the same transaction.

## Shop open-trades reads that name a contract

Workload: the item page's listing read and the checkout's store-mint and
coupon reads (`/v3/catalog/unified?contractAddress=&itemId=`), and
`/v3/catalog/shop?contractAddress=`, run on each item view and each purchase
confirmation. Each read builds the `unified_trades` CTE
(`catalog::unified_trades_cte_of`). When two UNION branches reference it
(`/v3/catalog/unified` without `source`), PostgreSQL materializes it before
the branches filter by contract; with one reference (`/v3/catalog/shop`,
`source=native`, `source=legacy`) it inlines the CTE. Materialized without a
scope, the CTE reads every open trade.

When a read names contracts (`contractAddress`, or the contracts of its
`items` selection: `shop_queries::named_contracts`; the item read by ids:
`shop_queries::id_contracts`), the CTE reads only the trades whose sent asset
is from those contracts. The partial index
`idx_trade_assets_sent_contract` on `shop.trade_assets (contract_address)
WHERE direction = 'sent'` finds them.

Invariants:

- The scope keeps every row the read returns. Each branch still applies its
  own contract and item filters. A trade row it keeps joins `shop.items` on
  `contract_address = contract_address_sent`, so its sent contract is one of
  the named contracts in lowercase. The rows are equal to the unscoped read's
  rows for that contract (the `market_tests` test
  `a_read_naming_a_contract_keeps_the_rows_of_the_whole_read`).
- Reads that name no contract (the feeds, related items, legacy and
  importable listings) keep the unscoped CTE.
- Write cost: each sent asset that the trades mirror or a local signed trade
  inserts adds one index entry. Received assets are not in the index.

The object pass builds `idx_trade_assets_sent_contract` in the migrate
transaction, not concurrently, so the trades mirror's writes to
`shop.trade_assets` wait for the build. To build it outside that window, run
beforehand, with the same definition as `schemas/30_shop.sql`:

```sql
CREATE INDEX CONCURRENTLY idx_trade_assets_sent_contract
    ON shop.trade_assets (contract_address) WHERE direction = 'sent';
```

The object pass creates only objects the database lacks, so it then skips the
index. A failed concurrent build leaves an invalid index of that name: drop it
before retrying.

## Camera reel World gallery lookup

World gallery requests resolve a name to scene IDs before reading public photos.
On a cache miss, Camera Reel reads enabled scene rows from
`explore.place_indexed` and active `explore.world_scene_place` identities joined
to their exact `(world_name, entity_id)` deployment in `worlds.world_scenes`
and their parent in `worlds.worlds`. A known identity takes precedence over a
catalogue row bearing its UUID, including a retired identity. Eligible identities
also resolve to their original catalogue IDs in the requested World, preserving
uppercase and `urn:uuid:` spellings for photos whose `place_id` retains them.
An alias cannot bypass the identity's eligibility checks. The World-name
aggregate is not a scene. Listing visibility does not control photo eligibility.
Canonical scene resolution excludes blocked Worlds and disabled scenes or
parents through `explore.place_moderation` and catalogue state.

These are read-only shared SQL edges through the existing pool. The active
identity index's World-name prefix, deployment key and moderation primary keys
serve the lookup; no extra projection or index is maintained. The result is a
sorted, deduplicated ID list held under the configured cache capacity and TTL,
so changes to scene eligibility may remain cached until expiry. GET and batch
POST use the same resolver; photo privacy, review and deletion filters apply
after resolution. Production lookup rates and latency have not been measured.

## Wallet rental reads

`GET /v1/rentals/{address}` reads `shop.chain_cursor` and
`shop.rental_event` in one statement snapshot. A single lookup returns
both wallet roles. The two partial participant indexes find the
wallet's historical assets in the requested venue, and the existing asset
index finds each candidate's latest rent or claim and the current period's
rent events. Each result contains one row per asset and role. There is no new
projection table or response cache. Production lookup rates and latency
have not been measured.

The missing-objects step of `schema migrate` builds
`rental_event_lessor_idx` and `rental_event_tenant_idx` non-concurrently.
It holds a SHARE lock on `shop.rental_event` until the migration commits,
so chain index writes wait. To keep writes moving during an index build on
a large history, prebuild both indexes with `CREATE INDEX CONCURRENTLY`
using the definitions in `schemas/30_shop.sql`.

The read stops at the published block. A stale or unpublished cursor
returns 503. The latest rent or claim is evaluated before checking the
current participants. A rent stays in the lessor's list until claim and
in the tenant's list until expiry. Extensions add to the end of the original
period, as the Rentals contract does. The route returns the operator on the
latest rent event. Mainnet and Sepolia use separate
`(network, block_number, log_index)` event keys and separate cursors under the
Ethereum supervisor. Only the mainnet index updates listing statuses and
supplies rental notifications.

The index worker uses the `TRADE_RPC_URLS` entry for chain 11155111 or
`ETH_RPC_URL` with `ETH_NETWORK=sepolia`. Check the selected chain's
`ready` field at `GET /signatures-api/v1/rentals-index` before routing
clients to the wallet rental endpoint.

## Rental listing status workload

`signatures.rentals.status` (`schemas/32_signatures.sql`) has three writers.
The `rentals` index fold (`shop/src/rentals/index.rs`, run by `catalyst index`
after each pass over `shop.rental_event`) moves listings to `executed`,
`claimed`, `cancelled` with `cancel_reason = 'nonce'`, or back to `open` on a
rebuild. The leased `rental-listing-status` job (`shop/src/rentals/status.rs`,
every minute on the fleet-jobs process) cancels open listings for three
reasons. It writes `expired` when `expiration` is past the bound clock. It
writes `transferred` when the land-authz index shows that the owner has
changed since the listing was created. It writes `dissolved` for an estate
without parcels once the index head is newer than the listing. The index
reads run only on a published land-authz snapshot. The refresh route
(`PATCH /v1/rentals-listings/{id}`) applies the fold and the same cancellation
rules to one listing. A third-party transfer must postdate listing creation.
Dissolution requires a membership observation after listing creation whose
block is at least the stored membership block. The refresh also compares the
metadata source block before using the asset to close a listing or update
its metadata.

Create and refresh bind the asset read to the signed or stored listing chain.
On a node walking its configured catalogue pair, they read the NFT index
publication and LAND rights in one database snapshot.
Incomplete mint or transfer times, an unavailable publication, or a document
beyond the LAND fold returns the unavailable envelope. A transfer beyond the
NFT publication also waits for metadata to catch up. The NFT publication identifies the served Ethereum chain. The walker uses published LAND to
authorize the direct owner and returns 401 when a third party owns the token.
It checks the observed LAND revision in the insertion transaction. During a
Rentals outage an existing open listing still returns 409. A listing by a lessor whose asset is in Rentals
custody requires a published Rentals cursor and a rent at or after the LAND
transfer into custody. PATCH rechecks a current custody observation when a
later Rentals publication names the lessor, even if its first refresh already
saved the transfer time. The status job reads the Rentals publication and
the rent in its LAND snapshot. A dissolution write locks the asset and
rechecks its membership version before closing the listing. The periodic job can
establish a third-party transfer or dissolution from published LAND alone.
Only a transfer verdict based on venue custody requires the Rentals publication.
Writes hold shared locks on the observed cursor rows until commit, so a concurrent
rewind or ordinary cursor update waits for the write transaction.

Nodes that do not walk the NFT index read listing assets
from the configured marketplace subgraph. The subgraph reports transfer time
in seconds and the block number and time of its estate membership snapshot. A missing or
unavailable subgraph returns the marketplace-subgraph unavailable envelope.
A walking node always uses its local publications. Each indexed asset lookup
reads the LAND and NFT publications in a single snapshot, probes the token,
its document and its first and latest transfers, and reads parcel proximity.
For an estate it also scans its member parcels to compute size and proximity.

Workload: mainnet holds thousands of listings, of which hundreds are open
(an estimate, not a measurement). Each pass does the following:

- One `UPDATE` over the open rows. The partial `rentals_open_asset_key`
  enumerates them, so no new index is needed.
- One snapshot select over the open rows of the LAND and Estate registries,
  with one primary-key probe into `land_authz.token_right` per row, one
  `authz_event_ownership_idx` probe for the latest transfer, one
  `estate_land_estate_idx` probe per estate and, while the chain-matched
  Rentals index is published, one `rental_event_asset_idx` probe for the
  asset's latest rent.
- One batched `UPDATE` by id, guarded by `status = 'open'`.

Invariants:

- `cancel_reason` is NULL unless the status is `cancelled` (a row `CHECK`).
- Step `0067_rental_asset_membership_block` adds the metadata source block
  and published LAND membership block. Existing rows, parcels, and imported
  metadata start at zero; the next indexed fill records the versions. Create
  and refresh compare both block versions independently of the legacy
  `nft_updated_at` timestamp, so a current indexed snapshot replaces
  preexisting subgraph-era metadata even when its event timestamp is older.
  `nft_updated_at` remains the greatest observed timestamp. Equal block
  evidence can refresh the row.
- The routes never move `rental_assets.nft_updated_at` backwards. Metadata
  details are guarded by `metadata_block` and `membership_block`, not by that
  legacy timestamp; an older published source version replaces no details,
  while the next current snapshot can replace imported or subgraph-era values.
  Create still creates the listing when its source version is older, and its
  upsert keeps the stored metadata. The refresh route skips an older source
  version. A reader that answers from an older block and then a less old one
  cannot pass the newer-row test. The signatures-server import resets both
  block versions to zero.
- A cancellation with no recorded cause keeps NULL. Rows imported from
  signatures-server, which records no cause, are of this kind.
- A reopening never makes a second open listing of an asset
  (`rentals_open_asset_key`). The fold writes reopenings after its other
  verdicts, reopens only the newest listing of an asset, and only while no
  other listing of that asset is open. A blocked listing keeps its stored
  status and reason.
- The fold reopens a cancelled listing only when its reason is `nonce` and
  its `expiration` is not before the fold's clock, the job's expiry test. An
  expired one keeps `cancelled` with `nonce`, and the asset's newest
  unexpired nonce cancellation reopens in its place. Any
  reason gives way to a rent of the listing: an incremental fold also
  revisits a cancelled listing whose own `rented` log is past its block. The
  fold reads the distinct signatures of the venue's `rented` rows past that
  block once and joins the venue's listings to them. On the measured data
  both custom and generic plans chose a hash join, so the leg cost one read
  of the range's rents plus one pass over the venue's listings, which the
  fold already scans, with no event lookup per cancelled listing. A set
  larger than the session's hash memory spilled to disk in batches and
  stayed linear. No test pins that plan; a statistics or planner change
  can choose another join. A rebuild and the refresh route never read that set. The refresh
  route, which folds one listing over every block, makes one
  `rental_event_signature_idx` probe for that listing's signature instead.
- The fold's `cancelled` verdict never rewrites a listing that is already
  cancelled. The fold reads its verdicts before it writes them, so a listing
  the job cancelled in between keeps the job's reason and stays closed on a
  rebuild. An `executed` or `claimed` verdict still applies.
- A cancellation that is not a nonce move is never undone, so an unknown never
  cancels. No published index, no indexed token, or an indexed transfer older
  than the listing all leave the listing open.
- For a token owned by the served chain’s Rentals contract, a published rent at or
  after the latest transfer into the venue identifies its lessor. Once the
  Rentals cursor covers that transfer, an absent or older rent establishes
  that nobody holds custody. Create returns 401 and a later transfer closes
  an open listing as `transferred`. While the cursor trails the arrival,
  create returns the `rentals-index` unavailable response and status leaves
  the listing open. On a Sepolia walker the ownership, venue, event network
  and published Rentals cursor all belong to chain 11155111. A graph-only
  Sepolia node leaves venue custody unknown.
- The signatures-server import keeps a node reason only while upstream also
  says `cancelled`, and clears it otherwise. It skips an upstream row older
  than the stored one, except that an upstream `executed` or `claimed`
  replaces a `cancelled` row of any age; the row then takes the upstream
  `updated_at`, which can be older than the cancellation's. Its load SQL is
  `tooling/migration/sources/signatures-server/rentals.load.sql`.

Step `0056_signatures_rental_cancel_reason` adds the nullable column and its
two checks without a rewrite.

## Camera reel photo events workload

`camera_reel.photo_events` (`schemas/50_create.sql`) is the append-only outbox
of the photo events camera-reel-service published to SNS: `photo-taken`, one
row per finalised upload, written in the transaction that sets
`camera_reel.images.ranking_published` (public and private photos alike), and
`photo-privacy-changed`, one row per visibility change, written in the
statement that changes it. `payload` is the upstream body `{type: "camera",
subType, key, timestamp, metadata}`. Writes follow the upload rate.

The Credits photo-goal worker reads committed `photo-taken` rows lacking a
receipt in `credits.photo_goal_receipts`. Receipt and effects commit together;
the outbox holds no consumer state. This anti-join also sees transactions
that commit after a higher event ID has already been processed. A sequence
ID alone is not a safe consumer watermark. The community photo counts are
applied directly in the upload's finalising transaction and do not read the
outbox.

Invariants: rows are never updated; the leased
`camera-photo-events-retention` job deletes rows older than 30 days, 1000 per
statement by `created_at` (its index), at most 100 statements an hourly pass,
so the table holds about 30 days of events and a consumer further behind
misses events. `user_address` is the photo's owner, lowercase. The bootstrap
defines the table; an existing database gains it from the object pass, so no
step exists.

## Seasonal photo rewards workload

`shop::credits::goals` reads Camera photo facts and Social subscription facts
using the existing database pool. `credits.seasons` and `credits.season_goals`
hold season windows, MANA limits and the `take-photo` assignment. These tables
have no active monetary seed. `credits.program_opt_outs` records enrollment
withdrawal separately from the paid wallet's identity.

The worker writes one durable `credits.photo_goal_receipts` row per processed
photo, including terminal ineligibility decisions. Eligible photos update
`credits.goal_progress`, keyed by wallet, season, week and goal. Completion
creates one `credits.goal_grants` row with immutable signature terms and one
keyed Social notification with its delivery records. All four effects share
one transaction. A failed signature or notification rolls back progress and
the receipt so the event can be retried. No network call runs inside this
transaction. Per-wallet advisory locks serialize progress and enrollment;
claim serialization enforces the weekly and season limits.

MANA amounts use integer wei in `numeric(78,0)` and exact integer arithmetic.
The signed grant binds wallet, chain, manager, amount, expiry and salt.
`credits.usages` supplies consumption by the same chain, manager and credit
hash. Goal grants never credit or settle the paid USD ledger. Wallet-scoped
reads use the progress and grant primary-key prefixes. The partial receipt
index on `(address, season_id, week_id, ordinal)` for counted photos serves
the ordered `photosTaken` lookup; the pending-photo query uses the unique
receipt identity. `METRICS.md` records the synthetic access-path measurement.
Production volume and latency remain unmeasured.

The `credits-reminders` job reads progress rows of the current week and grants
for its weekly reminders by the progress and grant primary keys. Its usage
reminders read claimed grants expiring within seven days or one day through
the partial index on `credits.goal_grants (expires_at) WHERE claimed_at IS NOT
NULL`, which the object pass creates on existing databases. Notifications are
keyed per wallet, season and week, so a rerun inserts nothing.

The worker runs through the existing leased job registry under the Ops write
gate. Receipts survive photo deletion and event retention; changing visibility
does not undo a reward. The Camera event retention window still bounds crash
recovery: a worker more than 30 days behind cannot recover deleted facts.
These new relations are created by the schema object pass on existing
databases, without resetting wallet, photo or notification data.

Credits keeps its own in-memory feature-flag snapshots, fetched with the
configured Credits audience outside SQL transactions. Social's persisted
`dapps` answer and Explorer's site answer cannot authorize these rewards.
Local flag overrides remain in `explore_api.feature_flags_local`; missing
remote policy is retryable rather than a completed photo decision.

`credits.captcha_attempts` holds a wallet's failure count and most recent
failure time. Three failures block claims for 24 hours from the last failure;
a successful solve clears the row. The existing `credits.captcha_challenges`
table records `generated_at` for the minimum solve-time check and keeps a
two-hour challenge lifetime. Both challenge and failure updates take the
same wallet lock. The additive `0043_credits_captcha_generated_at` step
preserves existing challenges and establishes their solve-time baseline.

## Camera reel import workload

`catalyst camera-reel import` (`camera_reel::import`) copies
camera-reel-service's `images` table into `camera_reel.images`. Inputs: the
source table read in pages of `--batch` rows (default 500) by its
`(created_at, id)` keyset, and two bucket objects per new photo, fetched at
most `--concurrency` (default 8) at once. The source has no index on that
order, so each page sorts the table's matching rows; this is a cost on the old
database during the copy window, unmeasured. Per page, one primary-key read
of the local rows (`id = ANY`) decides what is new; a photo already here is
not fetched again. Writes: per new photo, its blobs in the store, then one
transaction inserting the `images` row and its `camera_reel.imported_images`
row; per changed imported photo, one `UPDATE` of `is_public` and `metadata`
guarded by `review_status = 'ok'` and the provenance row. No
`photo_events` row is written and `ranking_published` is true, so the
community photo counts and the ranking repair never see an imported photo.

`camera_reel.imported_images` is the import's provenance: which local photos
the source may change or `--prune` may delete. The `tooling/migration`
camera-reel step writes it too, for each row it tracks in
`dcl_migration.camera_reel_image_loaded`, in the batch that loads the row. It is a separate relation and
not a column of `images`: it is a fact about the copy, read only by the
import by primary key, and the gallery rows stay as they are. Its foreign key
cascades from `images` (same schema, same lifecycle), so a delete of the
photo removes it. `--prune` collects the ids of one complete pass in memory
(16 bytes each) and deletes each imported row the pass did not read through
the owner-delete statement, which records the blobs in `withheld_blobs`. A
running `serve` loads that relation at boot, so a pruned photo's bytes are
withheld on `/contents/{hash}` after its next start; its reel route answers
404 within the servable memo's minute. The bootstrap defines the table; an
existing database gains it from the object pass, so no step exists.

## Profile snapshot repair workload

A profile may name avatar renders in `metadata.avatars[].avatar.snapshots`
that its `content[]` omits. Sync fetches them when a source has them and writes
a `content.content_files` row for each stored one (`{name}.png`, the reference
catalyst's key, for the names `face`, `face128`, `face256` and `body` only) in
the flush that writes the deployment; once that row commits, the blob
collector and the history prune keep the render. A render no source served
leaves no row, whether every source answered 404 or one failed (an error
status, a timeout), until a later version of the profile or the repair below
fetches it; with a single `SYNC_SOURCE`, one failed request is enough. Once
the entity is verified and holds its flush-backlog slot, each stored render
has its modified time set to now and is buffered with it, so the collector's
age floor covers it until that flush commits. A render the process may not
touch (a blob another uid owns) gets no row, with a `warn`. The history prune
has no age floor and the flush takes no store lock, so a render named only by
a pruned earlier version can be unlinked between the touch and the commit,
as a `content[]` file can, and the row then names a missing blob.

Any signed profile can name any stored hash among its snapshots, and the row
it gets keeps that blob from the collector while the profile is active, as the
reference catalyst's content-files fixer does. At most four rows per profile
come from snapshots, and the denylist still decides what is served.

`catalyst content repair-profile-snapshots` (`content::profile_snapshots::repair`)
adds those rows to profiles stored without them. Inputs: active profiles
(`entity_type = 'profile' AND deleter_deployment IS NULL`) read by `id` keyset
in pages of `--batch` (default 500), and their `content_files` rows by
`deployment = ANY` (served by the `(deployment, key)` unique index). No index
covers `(entity_type, id)` and no partial index holds the active profiles, so a
page either walks the primary key from the last `id`, reading every deployment
of every type, superseded ones included, until it has `--batch` active
profiles, or reads all profiles through `deployments_entity_type_index` and
sorts them. With the first plan a whole pass reads every row of
`content.deployments` once; the plan the server picks and the pass's duration
on a production-sized table are unmeasured. Per page, the renders the store
lacks are fetched from `SYNC_SOURCE`, at most 8 at once and within 64 MiB of
decoded bodies (a larger body downloads alone). Writes: per page, one
transaction that takes the store's shared lock, re-checks the blobs and
row-locks the page's deployments that are still active (`FOR KEY SHARE`, so a
concurrent history prune skips them or has already deleted them) and inserts
a row for each stored render of those, with `ON CONFLICT (deployment, key) DO
NOTHING`. A pass can be repeated
or interrupted; a render no source served is tried again on the next pass, and
a store fault ends the pass with an error.

## Badges catalog and progress workload

The catalog (`social.badge_definitions`, `badge_tiers`, `presence_badges`;
`schemas/40_social.sql`) is reference data: upstream's 34 badges and 84 tiers,
seeded by the bootstrap, a few dozen rows that change through the admin catalog
routes (below) or the TS database import. Every read route reads the published
rows in `created_at, id` order per request, as upstream does: one statement of
about 0.3 ms and 30 KB, so an import or a catalog edit shows at once
(`social/src/badges/store.rs`). Per
wallet, `user_badge_progress` holds one row per badge touched (its
`steps_done` plus upstream's `progress` document verbatim: the evaluators'
dedup and resume state, a few KB at most) and `user_achieved_tiers` one row per
earned tier; reads load both for one address in one statement.

Writers of progress go through `badges::progress::apply_progress` in their own
transaction: the badge row `FOR SHARE` only when published and not archived
(upstream's awardable rule, `published_at IS NOT NULL AND archived_at IS
NULL`), a transaction advisory lock per (address, badge) that also serializes
the console revoke, then the document, the counter, the newly reached tiers,
completion and, unless suppressed, one idempotent `badge_granted` notification
per new tier. The console grant raises the steps to the requested tier's
target through it. The backfill (`POST /badges/badges/{id}/backfill`,
`social/src/badges/backfill.rs`) runs one such transaction per wallet (a
statement error fails that wallet; a lost connection fails it and reports
every later wallet as not attempted, at once): it
takes the same badge lock and advisory lock first, merges upstream's way over
the progress it reads under them, writes through `apply_progress` without
notifications, then sets the merger's instants on the earned tiers and the
completion.

Invariants: `steps_done` equals the document's `steps`; earned tiers and
completion are never taken back by a write (only the console revoke deletes
them); a tiered badge completes at its last tier, a single badge at
`criteria_steps`; an archived badge stays listed and kept by its earners and is
never awarded; an unpublished one is invisible. A database stamped before step
`0037_social_badges_catalog_parity` keeps its four placeholder rows: the step
gives `open_for_business` and `emotionista` upstream's content and leaves
`decentraland_citizen` and `walkabout` unpublished. It also fits the old
progress rows to the new targets: a tiered row keeps completion only when the
wallet holds the last tier and its steps stop at its highest held tier's
target (no unearned tier on the next write), and a completed single badge
reads its target as reached. The rest of upstream's
catalog reaches it through the TS database import.

### Historical marketplace sale inputs

The Ethereum sales cursor watches accepted bids at the ERC721Bid contract, purchases at both names controllers, and changes to venue fees and recipients in `shop.sale_fee_change`. An accepted bid occupies one row in `shop.sale_event`. Each `NameBought` occupies one row in `shop.name_purchase` with its caller, beneficiary, name and fee in MANA base units. The original controller burns the price. The second pays its configured fee collector. Multiple purchases in one transaction have distinct log positions. The walk inserts its rows with its cursor advance. A rewind deletes all three tables' Ethereum rows above the retained block in the same transaction.

On Ethereum and Polygon, `shop.sale_event.bid_fee` records the integer emitted by `BidAccepted`. Older rows can have a null fee. Replaying the original event fills it, including an emitted zero. The off-chain marketplace walk keeps each executed asset's `extra` bytes in `shop.trade_event.assets`. On Polygon the MANA leg can carry the royalty recipients chosen during execution. A replay of the same transaction and signature fills these bytes on older rows.

On a populated deployment, the schema object pass creates the successful-order and accepted-bid indexes on the event tables. To keep writes available during the build, run these statements outside a transaction before `catalyst schema migrate`.

```sql
CREATE INDEX CONCURRENTLY IF NOT EXISTS nft_event_successful_order
ON shop.nft_event (network, block_number) WHERE kind = 'order_successful';
CREATE INDEX CONCURRENTLY IF NOT EXISTS sale_event_bid_accepted
ON shop.sale_event (network, block_number) WHERE kind = 'bid_accepted';
```

A Polygon store purchase ends at its `Bought` log. Each explicitly priced line matches one preceding `Issue` with the same transaction, collection, item and beneficiary, after the previous purchase in that transaction. Repeated copies keep their line order. After a complete replay of the purchase and its issuers, an unmatched line retains the explicit payment and has no indexed NFT sale. The sales cursor records historical changes to venue rates and recipients in `shop.sale_fee_change`. Rates take effect at their log position.

On an existing sale-event table, step `0064` creates the bid-fee check with `NOT VALID` to avoid a historical table scan under the migration lock. It enforces the check on every new or updated row. After the migration commits, validate historical rows in a separate transaction with `ALTER TABLE shop.sale_event VALIDATE CONSTRAINT sale_event_bid_fee_check`.

The accounting job reads three mainnet cursors per network. Step `0064_shop_sale_accounting` leaves their new decoder version and verified height empty on upgraded databases. After the live walk catches up, each cursor owner uses spare request budget for historical windows of up to 10,000 blocks. It checks the stored tip hash around each window. When the log provider narrows a range until the request budget runs out, the cursor saves the smaller upper bound for the next attempt. Incomplete economic evidence withholds sales coverage at that window while the primary index continues to advance. A Polygon repair reads Issues only from named factory collections in the Polygon catalogue. Raw Bought lines remain available for replay when a late collection is discovered. The sale fold uses the same membership check before deriving executions, payments or seller attribution. A rewind clips the verified height. A repair changes the history revision even if the tip stays at the same height.

`shop.sale_execution` stores one row per store purchase, accepted bid, successful order, off-chain execution or NAME fee. `shop.sale_asset` and `shop.sale_payment` store the ordered asset and consideration legs. The asset ID combines the network, transaction hash, execution number in that transaction and asset ordinal. A block or log position can change when the same transaction is included on a replacement branch. Store line prices and single-asset prices with one currency support per-item counts. A catalogue-only bundle without per-asset prices retains the ordered assets and consideration for reconciliation. Its payments and payouts are excluded from public sales totals. Trades containing foreign NFTs remain in the primary trade event log. NAME fees have their own payment and no asset sale.

`shop.sale_payout` records the integer amount and recipient of each matched payment leg. Receipts are checked against their transaction, block hash and venue log. The split covers seller, creator royalty, venue fee, store fee and any division remainder retained by an off-chain payer. A missing receipt or ambiguous match leaves `settled` false. A missing receipt is retried. A trade consisting only of existing ERC-721 tokens has its token IDs in the trade event and needs no receipt. A trade that mints an item still reads the receipt for its issued token ID, even when there is no payment. Store mints read collection creatorship at the receipt block and reverse later changes in the same block to identify the seller separately from the payee.

The fold groups payments by currency. UTC-day rows keep gross volume, counts, NAME fees, creator earnings, venue fees, store fees and burns. The original NAME controller and fees paid to the registered Ethereum MANA burners contribute to burns separately from venue fees. Item and wallet rows keep priced activity per day. Wallet totals use the wallet's days, and first-purchase rows date the first buyer, seller and item relationship for cumulative distinct counts. Recomputing a day replaces its rows and recomputes all affected wallets. A rewind or same-height repair records the earliest affected block. The fold keeps the preceding executions and replaces groups for both removed and replacement UTC days. A newly backfilled collection keeps the prefix before the earliest late collection. A change of source identity or a loss of collection coverage requires a new proof from the start. When no prefix can be reused, the first refold transaction removes the old executions and rebuilds every affected day and wallet total. Its locks and WAL volume grow with the previous history. Later passes replay the replacement history in bounded windows. [Full-refold follow-up](https://github.com/decentraland/dcl-one/issues/2287)

`shop.sale_publication` records the common block and hash and the three source identities and revisions. All three source tips must match the RPC branch. The accounting job locks and rechecks the input cursors before committing the facts and their marker together. Readers can obtain a read-only repeatable-read transaction for one network or both. Each requested network needs a recent whole-history publication, no newly owed history, matching source revisions and a recent accounting marker. The earliest of those freshness deadlines bounds use of the snapshot. A capped ordinary advance retains the previous freshness deadline until a near-head prefix earns a new one. Excluding an unlisted store issuer requires a complete Polygon factory cursor through the purchase. The fold saves the verified exclusion height on the factory cursor under the publication locks. A factory rewind clips that height to the surviving prefix. A rebuild clears it. A newly discovered issuer reopens repair even below a saved exclusion. The saved set in `shop.sale_covered_collection` bounds the next historical refold to additions, removals or changed backfill heights since that marker.

The sales publication identifies each asset by network, transaction hash, zero-based venue execution ordinal in that transaction, and zero-based asset ordinal in the execution. The execution ordinal counts raw successful orders, accepted bids, marketplace trades, distinct store Bought logs and NAME purchases by log order, before catalogue checks. Store Issue and transfer logs do not increment it. Re-inclusion preserves the relative order even when the transaction moves to a different block or its first log index changes. Adding a venue to this set requires a new publication version.

On a version 1 marker the fold ranks the persisted venue logs in the surviving prefix, stages the changed IDs, and replaces them in the same fenced transaction that publishes the new marker. The existing unique constraint remains valid throughout both updates. No historical receipts are needed for the ID repair.

Sales publication version 3 sums gross volume from positively priced NFT assets. A store payment belongs to its explicitly priced line ordinal. Multiple consideration payments for one trade NFT must reconcile to its price in the same currency. NAME registration fees are accounted for separately. When a network has a version 1 or 2 publication, the next fold identifies affected UTC days from local assets and payments, replaces their aggregates and account totals, and publishes version 3 in the same transaction. The routes require version 3 on each requested network. The upgrade reuses the existing executions and receipts.

A wallet-filtered sale with an unresolved seller withholds a response only when indexed evidence ties that wallet to the sale. Candidates come from the asset, the consideration beneficiary for its line, the item or collection creator, collection seeds and published creatorship transfers. An unresolved sale without a candidate stays out of address lookups. An unfiltered sales page that contains it returns HTTP 503. The fold logs a warning with the count of sales without a candidate on the days it recomputed.

The published sales readers use a 10-second PostgreSQL statement timeout. Recent unfiltered pages select asset keys in timestamp order and obtain the count from published daily rollups. Wallet lookups use indexes on the buyer, seller, payment beneficiary and collection item before enrichment. Filtered sales and proceeds pages cap the returned total at 5,000 or 25,000 for the requested page window. Sorting proceeds and checking unresolved seller evidence may still scan the seller's history. Seller summaries and store totals include the entire published history in scope.

Accounts, rankings and volume cache successful answers for 30 seconds across append-only advances. Each request checks the current publication and auxiliary sources in one snapshot. A rewind, replaced source identity, backfilled history or correction to a published sale changes the cache key. A withdrawn or expired required publication returns HTTP 503 even for a cached query. Warm hits bypass the query limits. Cold accounts and other analytics have separate limits of one and two concurrent reads. Sales pages have four. Seller summary, store stats, proceeds and the sales leg of activity share four slots. A request waits up to 250 milliseconds for a seller slot before opening the read transaction. Excess read requests receive HTTP 503. Activity logs when the wait expires and returns its available legs. The accounts address selector accepts up to 50 distinct wallets. Lifetime creator rankings require seller and settlement evidence for every relevant mint in the requested history.

Accounts read `shop.sale_account_total` for monetary totals. The sale fold credits net proceeds to the public seller, including the first consideration beneficiary of a mediated bid. Payout recipients retain their separate fee and royalty amounts. The publication upgrade repairs the UTC days containing affected bids from stored evidence before readers use those totals.

The special-execution index supports scans of store, trade and NAME-fee executions by network, kind and published block. The unresolved-asset index bounds the seller-evidence scans to priced bids and assets whose seller is missing. Token history uses the priced token lookup, with contract as a further constraint, and sale-type pages use the priced kind and block lookup.

The schema object pass creates the sales read indexes. On an existing large ledger, prebuild the definitions in `schemas/30_shop.sql` with `CREATE INDEX CONCURRENTLY` outside a transaction before running `catalyst schema migrate`. Inspect `pg_index.indisvalid` and `pg_get_indexdef(indexrelid)` for each prebuilt index. If a build was interrupted, drop its invalid index concurrently and rebuild it before the migration's name-based check.

### Badge processor

The writers of evaluated progress are four leased jobs
(`social/src/badges/jobs/`, on where `BADGES_PROCESSOR_ENABLED`, off by
default, and social writes are), each a reader of tables other areas own, with one row of
`social.badge_cursors` per source:

- `social.client_events` by `seq` (citizen, traveler, walkabout, moves
  master, social butterfly, vertical voyager), traveler resolving a parcel's
  scene title through `content.active_pointers` and `content.deployments`.
  The values are the client's claim, as upstream took them, within bounds:
  a `walked-distance` event counts at most 10,000 steps (the explorer
  reports on thresholds far below it), a `vertical-height-reached` height
  above 5,000 m is passed (no scene is that tall; one without a height, as
  the Unity explorer sends it, counts as reached) and a `passport-opened`
  receiver must be a wallet address. Times are Segment's `received_at`, as
  upstream stamped arrival, so a client cannot backdate or space its events.
  Segment stamps a whole batch with one `received_at`, so emotes or scene
  stays a client delivers late in one batch (after a retry or offline)
  collapse into one minute or one moment: a deliberate trade against
  client-set times;
- `content.deployments` by `id` (rarity badges, profile pro, land architect,
  the store half of open for business), a wearable's rarity from its active
  entity or `shop.items`;
- `shop.sale_event` (`mint`, `bid_accepted`), `shop.trade_event` (`traded`)
  and `shop.nft_event` (`order_successful`) on Polygon by (block, log index),
  each up to the block its index published in `shop.chain_cursor`, a
  cursor bound to chain 137 and out of quarantine (emotionista, fashionista; one step per transaction hash, a legacy order
  indexed before `nft_event.tx_hash` was recorded counted by its chain
  position until a re-index fills the hash). An off-chain good counts for
  its `beneficiary` in `trade_event.assets`; a legacy order bought by a fiat
  or cross-chain relay, a fee-taking order router or a CreditsManager counts
  for the recipient of the relay's own first `transfer` of the token after
  the order in the same block (transfer rows keep no transaction hash), and
  for nobody while a relay holds it (one `nft_event_token_idx` probe per
  such order). Step
  `0040_shop_nft_event_tx_hash` adds `tx_hash` as a nullable column without
  a default (catalog only, no row rewritten); it takes ACCESS EXCLUSIVE on
  `shop.nft_event`, held until `migrate` commits, so the nft index's writes
  and the reads of the table wait for the rest of the migration. A sale of
  an item `shop.items` lacks holds its log's cursor until the item is
  mirrored or the items mirror completes a pass an hour after the sale (a
  pass walks every item, minutes at the defaults); then it is passed without
  effect. Without
  `MARKETPLACE_MIRRORS_ENABLED` no pass ever completes, so the sales job is
  not started (logged at startup);
- recounts of `shop.items` and `shop.collections` on Polygon (once
  `shop.mirror_state` records a complete pass) and of accepted
  `social.referral_progress` rows (creators, the collection half of open for
  business, referral), for the wallets whose stored steps are behind. A
  creator's steps are the size of their item set, never below the stored
  count, so an imported item id in another format is not counted twice. A pass
  walks those wallets in address order and records the last one recounted
  (`badge_cursors.resume_after`), so a wallet the evaluator cannot move (an
  imported document whose flag is truthy but not `true`) never starves the
  ones after it; `position` counts completed passes. A batch reads its
  creators' items in one statement through `items_creator_lower_idx`
  (`lower(creator)`: the mirror lowercases, no constraint says so): 20 ms
  for 200 creators on 1M synthetic items (781 MB), where the unindexed
  per-creator read scanned the table once per creator. The candidate scan
  itself reads the table once per batch (150 ms on the same rows). The
  object pass builds the index non-concurrently in the migrate transaction,
  SHARE on `shop.items` until it commits (the mirror's upserts wait): 1.7 s
  on those rows; it can be
  built beforehand with `CREATE INDEX CONCURRENTLY` and the definition of
  `schemas/30_shop.sql`.

One transaction holds a batch (`BADGES_PROCESSOR_BATCH_SIZE`, default 200):
the cursor row `FOR UPDATE`, the batch's progress writes through
`apply_progress` and the cursor advance, so a database error rolls the batch
back and the next tick reads the same rows; a row an evaluator cannot read is
logged and passed. Invariants: a cursor never moves past a row it did not
pass; its first pass records the source's head and replays no history; a
recount's first pass writes no notification.

The id cursors (`client_events` by `seq`, `deployments` by `id`) never skip a
committed row. A sequence value is drawn before its transaction commits, so
ids become visible out of order: the sync flushes insert concurrent batches,
and a younger transaction can hold a lower id than rows an older one already
committed. A batch therefore passes every visible row past the cursor and
records each run of ids it passed without seeing in
`social.badge_cursor_gaps` (`lo..=hi`, the `horizon` and `seen_at` of the scan
that found it). The invariant: every id at or below a cursor's position is
processed or inside a gap of that source. Each batch first re-reads the gaps
and processes the rows that appeared (out of id order, as upstream's queue
delivered them), then drops a gap only when both hold:

- its `horizon` is at or below the `xmin` of a snapshot taken before the
  re-read that still misses it. The horizon is the transaction id of the
  batch that found the gap, which reads the cursor without a lock, scans,
  and only then locks the cursor row, which assigns that id: every
  transaction that had an id at the scan, so every one that could hold an id
  of the gap, has a lower one. (A snapshot's `xmax` is one past the highest
  completed id; a transaction still running can be above it.) Once `xmin`
  reaches the horizon all of them have ended, and a committed row would have
  been visible to the re-read. A batch that finds the cursor row changed when
  it locks it (a second runner after a lease expiry) writes nothing;
- it is older than the grace (600 s) and was then re-stamped. A writer draws
  the value inside its INSERT before it takes its transaction id (the row's
  default is evaluated before the heap insert assigns the id), so a writer
  between the two at the scan is not under the horizon. Usually that window
  is one row's projection, but an `ON CONFLICT` pre-check can wait in it on
  another open transaction inserting the same key. So the first batch past
  the grace replaces the gap's `horizon` with its own transaction id
  (`restamped`), and the gap is dropped only once `xmin` reaches that one
  too: every transaction such a writer could be waiting on has then ended.

Values that never commit (a rolled-back flush, a value an `ON CONFLICT DO
NOTHING` burned, as a concurrent duplicate Segment delivery does) are what
expire. A long
write transaction anywhere on the cluster delays expiry, never a row: rows
are processed as soon as they are visible. A source holds at most 5,000 gap
ranges; at the cap its cursor passes no row that would open another until
gaps resolve (it holds, it does not skip): alert on
`badges_processor_cursor_gaps` well before 5,000. The first pass starts at the
head, so a row still uncommitted below the head when a cursor is created is
history and not read. Each tick publishes, per source,
`badges_processor_cursor_lag` (ids behind the head; blocks behind the
published block for the sales logs), and for the id cursors
`badges_processor_cursor_gaps` and `badges_processor_cursor_oldest_gap_seconds`:
a cursor that stops advancing shows as a growing lag, and a horizon held by a
transaction left open on the cluster as gaps that keep ageing. A deployment stored
more than `BADGES_DEPLOYMENT_MAX_LAG_SECS` (default off, as upstream had no
such filter) after its entity timestamp is history a syncing node caught up
on and is passed without effect; `BADGES_EVENT_MAX_AGE_SECS` (default off) is upstream's
`EVENT_PROCESSING_MAX_AGE_IN_SECONDS`. Step `0038_social_badge_processor`
adds `seq`; the object pass creates `social.badge_cursors`,
`social.badge_cursor_gaps` and both indexes.

The `friend_first_wear` notification producer
(`social/src/notifications/first_wear.rs`, leased `friend-first-wear`) reads
`content.deployments` by `id` through the same id cursor and gap records,
under source `friend_first_wear`: the signer's `entity_timestamp` is not
storage order (a local deploy may be signed up to 15 minutes ahead, a synced
one long before), and a cursor on it would skip every profile stored after
one signed later and every tie past a page edge. Its first pass starts at the
head. A batch seeds baselines and claims first wears (`social.worn_history`)
and stores their notification obligations in `social.first_wear_outbox`
in the transaction that advances the cursor. The queue retains the wearer
name and URN independently of deployment retention. Its batch is
`BADGES_PROCESSOR_BATCH_SIZE` deployments, its gap grace the id cursors'
(600 s), and it publishes the same lag and gap gauges under its source. A profile the
node stored more than 24 hours after its entity timestamp is history a
syncing node caught up on and is passed without effect; the lag is measured
at storage, so a stalled producer still works through its backlog.
`social.first_wear_cursor` is no longer read.

First-wear intake pauses at 5,000 queued obligations. A final deployment batch
can cross this threshold. The queue continues to drain when intake is paused or
fails. The worker claims up to 16 due wears in one statement, ordered by
`(retry_at, address, urn)`. A two-minute reservation makes interrupted work
eligible again. It starts at most 50 pages and stops starting items after one
minute per drain. Each item's marketplace lookup has a 20-second timeout.
Losing the job lease cancels the pass.

Failed transport or timed-out lookups retry after the notification delivery
backoff (30 seconds, doubling per failure). The fifth failure ends the
obligation. Each drain expires obligations 24 hours after the wear's signed timestamp
or enqueue time, whichever came first. The worker logs expiry counts and pauses
in intake. Two consecutive transient failures on one network pause lookups on
that network for the rest of the drain, leaving the other network eligible. Invalid reader URLs,
invalid subgraph responses, missing readers, unknown or unbuyable items,
no active friends and suppressed recipients also complete without a notice.
The wear stays in history after completion. External reads run outside the
completion transaction. A transaction advisory lock serializes recipient rate
checks with notice insertion, delivery intents and removal of the obligation.
The foreign key cascades removal of a history row to its obligation.

### Presence badges flow

Input is every verified LiveKit join and leave (the gatekeeper's
`RoomEventStore`), including one the gatekeeper's own session ordering
supersedes, such as a reconnect's old leave that follows the new join: wallet
identities only, scene and world rooms only, with LiveKit's participant sid as
the session. The webhook inserts each into `social.presence_events` before it
answers, keyed by LiveKit's event id, so a redelivered webhook is one row; a
failed insert answers 503 and LiveKit delivers the event again. Only events in
a place a badge could count now are kept: a 5 s snapshot of the published badges' worlds, parcels and the
scene entities deployed on them (one GIN-indexed `entity_pointers &&` read),
with the engine's lookahead and lookback. A scene entity the snapshot does not
know (a redeploy since) is looked up by `entity_id` in the webhook that carries
it, under the webhook's lock stripes, and dropped
only when it is deployed off every badge parcel; one the node has not synced
is kept for the engine, which leaves it queued (and the wallet's later events
behind it) until the scene syncs, trying it again after 5 s, doubling up to
5 minutes, and matching no parcel after an hour or at once while 1,000
events already wait (so made-up scene ids bound the retry work). Waiting
events are counted once (`scene_not_synced`), gauged as
`badges_presence_waiting_events` and left out of the oldest-age gauge.
Every event is kept when the snapshot cannot be read.
A badge published or moved earlier is seen within 5 s; joins before that are
not counted, and the player's next join is. Expected volume is every
join and leave of the scene and world rooms of active presence badges
(unmeasured); rows other than waiting ones live only
until the next pass of the leased `badges-presence` job (5 s), which applies
up to 500 in `seq` order, one transaction per event that deletes it, so the
queue is its own cursor and a row committed behind a later `seq` is still
there on the next pass. The clock (bookkeeping times and the 5-minute future
guard) is read per event, so a long drain stays current. Only a deterministic
failure counts toward dropping an event (after 3 passes, logged and counted);
a connection, pool, timeout, conflict or failover error is retried on the next
pass without counting until the event has been queued for an hour, then counts
too, so one event cannot hold the queue. `badges_presence_queue_oldest_seconds`
reports the age of the oldest queued event of a wallet not waiting for a
scene, each pass (one `presence_events_seq_idx` scan that stops at the first
such row); a failed read keeps the last value, counts in
`badges_presence_queue_age_read_failures` and does not fail the pass.

Every presence transaction first takes one advisory lock: the engine and the
`badges-presence-sweep` job are a single writer fleet-wide, so the runtime
tables need no per-session locks. The runtime state is upstream's
(`presence_visits` keyed by badge, wallet and room, cascading from
`presence_badges`; `presence_applied_events`, `presence_pending_leaves` and
`presence_absence_facts` keyed by their facts), in epoch milliseconds where a
value meets a LiveKit event time or the `intervals` document. Indexes follow
the statements: `(wallet, realm_name)` for a leave's open visits,
`opened_at` and `(last_checked_at, opened_at)` for the sweeper's due visits,
and each fact table's retention column for the bounded prune (batches of
`BADGES_PRESENCE_PRUNE_BATCH_SIZE` rows, 1,000 by default, per table until one
comes back short, at most 20 per sweep, after the redelivery window,
`BADGES_PRESENCE_APPLIED_EVENTS_TTL_SECONDS`, 7 days by default). Completed badges are left out in one statement per event,
so a wallet with nothing left to earn in a place records no visit or pending
leave there. A scene badge resolves
the event's entity to its parcels through `content.deployments` (`entity_id`
is unique); a world badge compares names. Progress is written through
`apply_progress` with the progress document `{ steps, intervals }` kept as
upstream writes it, so an imported document resumes. As upstream's catalog
requires, a badge is counted only when its row is single (not tiered), its
`criteria_steps` is its rule's (`required_seconds` for a stay, 1 for a visit)
and no evaluator or import of its own (other than a one-time import) writes
it; another row is skipped and counted as `badge_invalid` (logged once per
badge and reason), so the
flow never overwrites another writer's progress. Invariants: a session
counts only from its own join to its own leave, never past the first
occupancy observation that missed the wallet; an ended session never reopens
for that badge; overlapping connections count once; a stay completes at the
instant the union reaches `required_seconds`. A grant writes its
`badge_granted` notification unless `BADGES_PRESENCE_GRANT_NOTIFICATIONS` is
false. The sweeper reads this process's LiveKit occupancy observation
(`comms::presence`) and only closes visits; no observation, or an empty one,
changes nothing. `BADGES_PRESENCE_SWEEPER_ENABLED=false` stops only these
occupancy checks; the maximum age and the prune still run.

Upstream's tuning is environment settings, in seconds and with upstream's
defaults (shared with `presence::Settings::default()`; each duration at most
30 days, each batch at most 100,000):
`BADGES_PRESENCE_EARLY_JOIN_LOOKAHEAD_SECONDS` (21,600; 0 keeps no early join),
`BADGES_PRESENCE_OPEN_VISIT_CHECK_AFTER_SECONDS` (900),
`BADGES_PRESENCE_OPEN_VISIT_MAX_AGE_SECONDS` (86,400),
`BADGES_PRESENCE_APPLIED_EVENTS_TTL_SECONDS` (604,800),
`BADGES_PRESENCE_ABSENT_EXPIRE_AFTER_SECONDS` (3,600),
`BADGES_PRESENCE_SWEEP_INTERVAL_SECONDS` (60), `BADGES_PRESENCE_SWEEP_BATCH_SIZE`
(100), `BADGES_PRESENCE_PRUNE_BATCH_SIZE` (1,000),
`BADGES_PRESENCE_GRANT_NOTIFICATIONS` and `BADGES_PRESENCE_SWEEPER_ENABLED`
(both true). Upstream names them without the `BADGES_` prefix and gives the
sweep interval in milliseconds.
### Adding a badge

A running node takes a new badge through the bearer-gated catalog routes
(`social/src/badges/catalog.rs`, bearer `CATALYRST_BADGES_ADMIN_TOKEN`, writes
refused while `SOCIAL_WRITES_ENABLED` is off). A schema step cannot insert
catalog rows, and the bootstrap seed reaches only new databases.

1. `PUT /badges/badges/{id}` with the definition in the shape `GET
   /badges/{id}` reads: `name`, `description`, `category` (upstream's enum),
   and either `criteria: {steps}` (a single badge) or `tiers: [{tierId,
   tierName, description, criteria: {steps}}]` in tier order (a tiered one,
   steps strictly increasing). `assets` (`{"2d": {...}, "3d": {...}}`, paths
   relative to `BADGES_PUBLIC_ASSET_BASE_URL`) defaults to upstream's CDN
   layout: `<id>/{2d,3d}/...` for a single badge, `<id>/<slot>/{2d,3d}/...`
   for a tier whose id is `<id with _ as ->-<slot>`; any other tier id names
   its assets. Every problem is one 400. A new badge is created unpublished
   and invisible (201); a repeated `PUT` replaces the content in place (200),
   keeping publication, archive and `created_at`. A `PUT` that would remove a
   tier a wallet holds, or switch a badge with player rows between single and
   tiered, is a 409 and changes nothing; a removed tier nobody holds is
   deleted from `badge_tiers`. Player rows are never written: a lowered
   target awards on the wallet's next progress write (for a processor badge
   other than traveler, the next one that gains steps, one tier per step or
   event), not retroactively, and a tier added above a completed badge
   leaves it complete until such a write.
   A badge takes at most 32 tiers, each `2d` or `3d` at most 8 files, and
   the badges routes refuse bodies over 1 MiB (413), the backfill's over
   64 MiB and an art upload's over 16 MiB.
2. Make the art reachable: upload it to the CDN that
   `BADGES_PUBLIC_ASSET_BASE_URL` names (upstream's by default), or, when the
   base is `<node>/badges/assets`, put each file into `BADGES_ASSETS_DIR`
   under the same relative path: mirror it there, or `PUT
   /badges/assets/{path}` with the admin bearer and the PNG as the body (204;
   only a path the badge or one of its tiers names, else 404). The upload is
   written on the process that receives it, so every process serving the
   badges routes must mount the same directory (a shared volume).
3. `PATCH /badges/badges/{id}` `{"published": true}`. The badge is listed and
   awardable. An active badge with no evaluator, backfill import or presence
   configuration (`social.presence_badges`) is named in a warning at startup,
   since only console grants award it.
4. Grant it per wallet with the console grant (`POST
   /badges/users/{address}/badges/{id}`), which awards it through
   `apply_progress`.

Retire a badge with `PATCH {"archived": true}`: it stays listed and kept by
its earners, and `apply_progress` answers `NotAwardable`. `{"archived":
false}` reopens it. `{"published": false}` hides a badge and is a 409 once
any wallet has a row of it. Each upsert (with the definition it wrote) and
each publication or archive transition appends a row to
`social.badge_catalog_audit`; `badge_admin_audit` keeps per-wallet grants and
revokes, whose `address` a catalog write does not have.

## Market chain index cursors

`shop.chain_cursor` holds one row per projection: an index (`nft`,
`trade_events`, `rentals`, `rentals_sepolia`, `mana_transfers`, `sales`, `committee`,
`third_party_registry`, `collections`) on one network (`ethereum`, `matic`). A window commit writes it once, and so do a
fold record, a publish and a rewind; the table stays at a dozen rows.
`shop.chain_checkpoint` holds each projection's recovery points (see
"Reorgs").

A catalogue database serves one Ethereum/Polygon pair. The NFT worker uses
`MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM` and `MARKETPLACE_INDEX_CHAIN_ID_MATIC`
to select mainnet (1/137) or Sepolia/Amoy (11155111/80002). Its RPC must
serve that chain. The mirror posture derives the testnet defaults from
`ETH_NETWORK=sepolia` when the operator leaves the IDs unset. At startup
the catalogue writer checks the pair against
`ETH_NETWORK` and the database binding. The API process leaves the binding
to the index writer, even when they share settings. Each database can have independent
Ethereum and Sepolia Rentals histories.
The `nft_catalogue_pair` row in `shop.worker_progress` identifies the pair.
`shop.mirror_state.chain_id` identifies each mirror's completed chain-bound
copy. A new source starts from page one. For testnets the mirror
checks the graph head on the configured chain by comparing a confirmed block
hash against the chain-checked RPC. Both mirrors persist that hash with their
keyset and read every page at that block. Collections supply contract addresses
and items supply their metadata. A mirror writer can provide this proof even
when the NFT walker runs on another node. A writer without a catalogue RPC
leaves a bound pass untouched. A missing or mismatched page hash restarts
the pass. A transport or GraphQL error retries the same block for up to an
hour from the start of the pass. A progressing pass can keep its pin for
one day. Descriptive readers can use the last completed copy during retries.
The NFT walk requires a complete collections pass for its chain. Mainnet
accepts a completed legacy copy with neither a chain binding nor a pending
testnet pin. The testnet walk selects addresses from rows the verified pass
actually fetched. Rows retained from a graph-only source carry no proof.
An empty verified pass covers the chain's
fixed contracts. The Sepolia/Amoy pair omits the mainnet-only committee, collections, third-party registry and MANA transfer walks, even
when the mirror posture enables them and mainnet RPCs are available. The
separately keyed Ethereum Rentals histories keep their own endpoints.

A Sepolia catalogue whose mirrors are bound to the mainnet default URLs
can be rebound when it has a Sepolia LAND cursor. Check the intended database
and the URLs in `shop.mirror_state`, stop its catalogue writers, and
configure the Sepolia and Amoy graph and RPC sources. Run
`PGOPTIONS="-c rebind.confirm_testnet=yes" psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f tooling/migration/rebind-legacy-catalogue-to-testnet.sql`
on that database, then restart its writers. The command logs the rebind,
clears both descriptive mirrors and their publication, and records the
testnet pair in one transaction. A database with an existing pair binding,
NFT history, or other mirror sources refuses the operation. A catalogue
writer always refuses a conflicting pair on ordinary startup.

Marketplace NFT and NAME readers select the node’s chain pair from its
configuration. Walkers require complete, fresh, chain-bound folds and read
them alongside their rows in the page snapshot. A walking cursor may be
ahead of that fold. Authorization also checks for pending relevant events.
Non-walking nodes use their configured graph reader. A descriptive item or
collection mirror accepts legacy unbound copies and checks every recorded
chain binding against the pair selected by `ETH_NETWORK`. The Amoy address
book contains no LAND, Estate or NAME registry. Those filters on Amoy
return an empty page.

On testnet a catalogue item used to describe an owned NFT must carry its
own verified-row flag. Migration 0072 keeps the descriptive mirror copy
while restarting any testnet item pass. A complete pass from a writer
that sets the flag establishes reader readiness, including an empty pass.

On Sepolia the NFT and LAND authorization walks start at LAND proxy
deployment block 3,831,219. On Amoy the NFT walk starts at marketplace V2
block 5,706,656, before the collection factory. The historical estate
repair below runs only on Ethereum mainnet.

The Sepolia bid walk and the Amoy bid and mint walk publish raw sale
events. Economic routes on testnet keep their configured authority. Raw
coverage cannot establish sale prices, aggregates, tax, royalties or
historical settlement terms. The separate sales accounting publication
runs on mainnet.

On Amoy the sales projection reads BidV2 and prices CollectionStore mints
against Bought logs from the same store and transaction. Its venue check
seeds missing Amoy fees on an upgraded database. Step
`0071_shop_testnet_sale_and_trade_venues` withdraws
the old Sepolia trade publication once. Its worker rebuilds from the V1
deployment at block 6,770,056. V2 and V3 trade verdicts share the same
publication and answer the trade-events-index unavailable envelope during
catch-up. The migration fences the old owner. A quarantined legacy cursor
keeps its start and stays unpublished. After Revalidate, the walker proves
its stored checkpoint and schedules the V1 replay. Binaries without the V1
filter cannot claim this cursor after a rollback. Restart a V1-capable writer
to resume the Sepolia trade walk. A partial replay with a custom start keeps that
start and needs the matching configured override. A revalidation of V1 history
reuses its canonical checkpoint. Mainnet and Amoy trade histories keep
their existing publication.

`shop.land_data` stores the latest parcel Update and the latest valid estate
document. A malformed parcel Update clears the fields at its log position.
The Ethereum NFT walk reads estate names at creation and on update. After
an upgrade, a published cursor can already be past creations that the old
walk skipped.
Using spare log budget after publication, each pass reads one window of
`CreateEstate` from the Estate registry, starting at deployment block 6,236,547.
The `nft_estate_creations` row in `shop.worker_progress` records the last
repaired block, its hash, and the original cursor height. Each archive window
is checked against the published cursor and the hashes of its creation logs.
The marker also records the forward NFT checkpoint and the cursor's rewind
count. In its worker_progress arrays, key_ceiling[1] and [2] hold the
forward block and hash; key_position[1] and [2] hold the archive predecessor
hash and the rewind count. An upgraded writer moves both in its fenced cursor
transactions.

The repair intentionally writes shop.land_data even though land_authz also
stores estate documents. Existing estate and parcel readers consume the
published NFT projection from shop.land_data; using land_authz here would
require changing those readers and would leave the historical gap dependent
on an operator replay. The repair instead self-heals that projection after
publication without delaying ownership or profile reads.

Historical names keep their existing values until the final window rebuilds
the estate rows by block and log index, raising the nft cursor's `rebuilds`
in the same transaction (the explorer map's build watches it). Estate documents with valid empty
fields are stored as empty strings, including descriptions; rejected
documents do not clear the previous valid estate data. Estates with only
rejected documents then have no row. Ownership and profiles remain available
during the repair. A rewind removes orphaned events and shortens the repair
range before the walk resumes. If the rewind removes the remaining archive
windows, it reconciles the retained estates in its own fenced transaction.
Migration 0062 installs a reset trigger for the repair marker. It invalidates
the marker only when the Ethereum NFT cursor completes a full rebuild reset to
its start block; ordinary reorg rewinds preserve the repaired prefix and are
reconciled by the walker.

The default window is at most 10,000 blocks. The historical prefix is roughly
17 million blocks, so it takes at least about 1,700 successful repair passes;
archive retries and dense creation blocks can extend that time. A pass runs
one archive window only when its log budget has room, and uses the remaining
budget to cap extra per-creation block-header reads. The safety bracket still
uses the chain id and up to eight header/window reads, so a repair can exceed
the leftover log budget by that bounded amount. A provider may internally
split one eth_getLogs window into several RPC requests as well. Failed or
timed-out repair attempts are retried and omitted from the pass request count.

An operator can see completion when the marker's position equals its ceiling:
SELECT position, ceiling FROM shop.worker_progress
WHERE worker = 'nft_estate_creations' AND scope = 1;
The final registry replacement commits in the same transaction that reaches
equality. A missing marker means the next published pass must initialize or
reopen the repair.
With `INDEX_METRICS_BIND` set, each index process reads these bounded cursor
tables once per minute. It samples the height, stored history mode, fold
publication and quarantine from `shop.chain_cursor` and the height, replay
mode and fold from `land_authz.index_cursor` for its configured registries
and network. A second read samples complete-pass times and page counts from
`shop.mirror_state` and `shop.trades_mirror` for the configured sources.
The sampler uses the index worker's existing pool and writes no rows.

**Ownership.** Each network has a supervisor job, `chain-walk-ethereum` and
`chain-walk-matic`, in the process that runs the indexes. It walks each
projection that process enables under the projection's own lease,
`<index job>-<network>` (`nft-index-matic`, `trade-events-index-matic`,
`rentals-index-ethereum`, `rentals-index-sepolia-ethereum`), so
processes with different projections enabled share a network. Each
projection keeps its own cadence and backoff, though one that reached the
target may join a due companion's shared walk ahead of its own turn, so
projections walking together go at the faster cadence. A process that runs no fleet
jobs (`FLEET_JOBS_ENABLED=false`) takes no lease and walks nothing.

The projections a supervisor holds that read the same endpoint and stand at
the same checkpoint (block, hash, whole history) walk their windows
together: each window is read once for all of them, and each projection
takes the logs its own filters take and commits them with its own cursor
advance, in its own fenced transaction, nft before sales. Each closes (folds
and publishes) on its own. A window read they share that fails sends each
back to its own checkpoint, alone, with what is left of its budget, and its
walk after that is alone too, on a whole budget. A read they all need first
(the chain id, the checkpoint's header, the head and the finalized block) is
tried again once after half a second; if it still fails, each one's attempt
fails and waits out its own backoff.

The supervisor claims a projection on the session that holds its lease. The
claim raises `owner_generation` in one bounded transaction
(`statement_timeout` 2 s, `lock_timeout` 1.5 s) and creates the row when
there is none. The supervisor keeps that generation for the lease
acquisition. A lost lease ends the projection's pass at once, even inside a
fold; transactions it had already committed stay, and the generation fences
any later one.

Every transaction that writes the projection ends by locking the row
(`SELECT … FOR UPDATE`) and comparing that generation, the chain binding and
the quarantine. On a mismatch it rolls back, so a job that lost the
projection commits nothing. This covers:
- the window's rows with the cursor advance (which also requires the
  checkpoint it read, height and hash);
- a reorg rewind, and the withdrawal of a diverged checkpoint's publication;
- the nft backfill lane;
- each fold, including each trade-events digest batch;
- the fold record, the publish and its withdrawal.

The lock is taken last, so it lasts only as long as the commit. A writer
that also moves the notification producers' cursors (a rewind, a rebuild,
the nft lane) moves them before it locks this row. A fenced
job drops its generation, and its next pass claims again, which only works
while it still holds the lease.

**Binding.** `chain_id` is the chain the history was read from, as the
source proved it at the claim. The walk checks the source's chain every
pass; a source of another chain fails that projection's pass alone, which
backs off on its own. A row from before the binding (`chain_id` NULL) is
adopted at the start of a pass, before any lane, rewind, fold or publish,
in two cases:
- it has no history yet;
- its stored last-block hash equals the source's hash at that height.

Adoption is a compare-and-set on the generation and the checkpoint.

**Quarantine.** A history that cannot be proven is set apart, with a
reason, and its publication withdrawn:
- a hash that differs;
- history with no stored hash;
- a binding to another chain.

A source that does not answer decides nothing: the pass fails and is
retried. A row in quarantine is not walked, rewound or published.

**Readiness.** Readers trust a projection only when all of these hold:
- the history is whole and `folded_block = last_block`;
- `folded_at` is within 30 minutes;
- it is bound to the chain the reader expects and is out of quarantine.

The NFT and trade-events walks use the configured catalogue pair. NFT reader and
status readiness expect mainnet publications (1/137). The
Rentals histories have separate mainnet and Sepolia cursors. The other
walks expect chain 1 or 137. The readers of `trade_events` expect the chain the node
configures (`MARKETPLACE_INDEX_CHAIN_ID_*`): the Rust readers and the
notification producers pass it, and its verdict fragments in the listing
SQL read the chains the process serves (`catalog::served_chains`). The
transfer notices match the nft and trade-events indexes at the same
heights, so they run only when both read Polygon mainnet. The tips' global
cap evicts by the MANA index's newest block only while that cursor is bound
and out of quarantine.

**Reorgs.** Each window is read inside a bracket: the header at its end
(`to`) before and after its logs, and between them the header at the block
the window actually reached (an endpoint may narrow it) and the header at
the checkpoint it extends. The walk assumes an endpoint that answers from
one branch between two equal answers for the header at `to`. Under that
assumption, the reached block's hash names the branch the logs came from,
and the predecessor proves the window extends what was committed; a hash
cannot attest logs from an endpoint that mixes branches. The nft backfill
lanes read below the cursor, each read bracketed by the cursor block's
header, which must stay its stored hash; each lane window's closing read
opens the next. A window that extends a hashed checkpoint costs one header
read more for its predecessor, and N lane windows in a pass cost N + 1
header reads besides the pass's own check of the cursor block.

Each advance records the block it reached, with its hash and time, in
`shop.chain_checkpoint`, keeping the newest 64 per projection: per
committed window, one delete of the points past the checkpoint it extends,
one upsert and one delete below the newest 64; a few hundred rows in all,
read only once a reorg is seen.

A pass checks the cursor block's hash first. When it differs, or when a
window's predecessor or a lane's bracket stands still on another branch
than the stored hash, the publication is withdrawn at once, in a fenced
transaction of its own on that exact checkpoint, and `diverged_at` is set;
the rows, the recovery points and the notification cursors stay. Readers
that use a cursor's height without its publication skip a diverged one, in
the statement that reads its rows: the sales fold infers no seller from the
nft walk's transfers, and the tips' global cap evicts no other sender's
claim against the MANA index. The rewind clears `diverged_at`; when the
chain flipped back before any rewind, the next pass that finds the cursor
block on its stored hash clears it. The pass then reads the headers
of the recovery points below the cursor, newest first and outside any
transaction, and rewinds to the newest one still on the chain, keeping its
hash and time. With none recorded, or every one compared and none on the
chain, it rewinds to `start_block - 1`. A header read that fails rewinds
nothing, and the next pass searches again. The search's reads count in the
pass, but its budget does not cut them. A shared walk never searches: a
member that diverged withdraws and recovers in a pass alone, which runs
under no member deadline, and walks with others again once its checkpoint
matches theirs.

The rewind is one transaction: the notification producers' replay plan,
the projection's own cleanup past the point, the producers' moves, the
cursor's compare-and-set from the checkpoint it read, the pruning of the
recovery points past the point, then the owner check. The next fold is a
rebuild, as after any rewind.

A sales fold may have inferred a bid's seller from an nft transfer before
the nft walk saw the fork. So the nft walk's rewind on Polygon (also a
rebuild) records its point in `shop.bid_seller_recheck`, in the rewind's
transaction, with the deletion of the transfers past it: a fold that reads
the row also reads the rewound transfers. The next bids fold reads those
rows, infers again the seller of every bid created past the lowest point (a
seller a sale event names stays), then deletes the rows it read; a row
recorded meanwhile waits for the next fold. The table holds one row per nft
rewind until a sales fold runs. After a rebuild the point is the start, so
every seller inferred from a transfer is unset and comes back as the nft
walk passes its bid again; nothing outside the bids fold reads
`nft_bid.seller`.

A database that gains the table starts with no recovery points. A search
reads only the points below the cursor, and the first advance's point is
the cursor itself, so a reorg seen before the second advance rewinds to the
start, as after a bootstrap or a rebuild. The history it holds is trusted
as found: the bracket certifies what is walked from then on, not rows
already written. Each advance also drops the points past the checkpoint it
extends, which no proven prefix holds.

**Operator procedures.** Run each as one statement. Both raise the
generation, which fences the current owner's writes in flight; its next
pass claims the projection again and acts.

- *Revalidate*, after a wrong endpoint was fixed. The history is kept and
  the next pass proves it again:

  ```sql
  UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL, chain_id = NULL,
         owner_generation = owner_generation + 1
   WHERE indexer = '<indexer>' AND network = '<network>' AND quarantined_at IS NOT NULL;
  ```

- *Rebuild*, a history that cannot be proven. The publication is withdrawn
  at once. In one transaction, the owner's next pass:
  1. plans the notification producers' replay from the state before the
     cleanup;
  2. runs the projection's own cleanup down to `start_block - 1`;
  3. restarts the cursor there, bound to the chain it proved and out of
     quarantine, and drops the projection's recovery points.

  It then walks and folds again; the other network and the other
  projections are untouched. Trade events also drop their derived verdict
  rows, so every digest is settled again from the stored trades (see "Shop
  trade verdicts").

  A rebuild keeps the projection's chain and start block. It is refused in
  three cases: the cursor's history is from another chain; it starts at
  another block than this node's walk; or it has no binding and its
  checkpoint hash is not the source's at that height. Then the cursor stays
  in quarantine with the reason, and nothing is deleted, not even the
  notification producers' positions. A source that does not answer decides
  nothing. A database for another chain begins empty:

  ```sql
  UPDATE shop.chain_cursor SET rebuild_requested_at = timezone('UTC', now()),
         folded_block = NULL, folded_at = NULL, owner_generation = owner_generation + 1
   WHERE indexer = '<indexer>' AND network = '<network>';
  ```

**Deploys.** A binary from before step `0054_shop_chain_cursor_ownership`
does not check `owner_generation`, and one from before the per-network
supervisors takes leases named after the index alone (`nft-index`), which do
not exclude the new ones. Stop the process that runs the indexes
(`catalyst index`) before starting a newer one, and before rolling back. A
deployment that runs the index process as one replica replaced with the
`Recreate` strategy already does this. A binary from before
`shop.chain_checkpoint` neither records nor drops recovery points, and its
rewinds and walks can leave a point whose hash names a branch other than
the rows below it. After running one, and before a newer binary starts
again, clear them: `DELETE FROM shop.chain_checkpoint;`. (A newer binary's
advance drops only the points past the checkpoint it extends, which does not
cover a point below the cursor.) Such a binary also leaves `diverged_at` as
it finds it; the newer binary's next pass that finds the cursor block on its
stored hash clears it.

The schema migration object pass adds the Ethereum NFT cursor's estate
repair trigger. It clears the marker when a writer completes a requested
rebuild by resetting the cursor to its start. During a reorg the upgraded
writer shortens the archived prefix and records the new rewind count.
An older writer can advance past missing creations, or rewind and replay
to the very same height and hash. On upgrade, a checkpoint or rewind-count
mismatch automatically reopens the historical prefix through that tip.

## Shop trade verdicts

Workload: each trade-events fold, every pass of the walk on each network.
It first settles the V3 digests, then folds every touched trade into
`shop.trade_chain_state`. Every route over `shop.trades` reads the verdicts.

A V3 marketplace keys a cancellation on the trade's EIP-712 digest.
`shop.trades.trade_digest` stores the digest the signature was verified
over: `POST /v1/trades` writes it, and the marketplace-server import copies
upstream's. The mirror cannot copy it, because upstream's API does not
expose it.

The digest step selects at most 2,000 V3 trades of the network per
statement: those without a verdict row, and those whose row has no digest
and no failure newer than 10 minutes. For each trade it takes the stored
digest when the signature recovers the signer over it. Otherwise it rebuilds
the digest from the row on the same condition. A trade that gains a digest
is refolded (`folded_block = -1`). A trade that cannot get one is marked
`digest_failed_at`.

Invariants:

- A digest in `trade_chain_state` recovers the trade's signer. A verified
  digest is never overwritten (`WHERE digest IS NULL`).
- `digest_failed_at` is set only while `digest` is NULL (a CHECK). A database
  error during the step aborts the fold and marks nothing.
- Each statement's trades leave the candidate set, with a digest or a failure
  newer than the backoff, so the step's loop ends. A permanent failure costs
  one rebuild per trade every 10 minutes.
- The open-listing reads (`unified_trades`, `/v1/bids`) list a trade only
  while its network's trade-events cursor holds a publish
  (`chain_walk::published_sql_on`: a fold published within 30 minutes; a
  pass in progress keeps the previous publish), in the same statement. A
  trade that is sold, cancelled or marked is not listed. A marked trade
  still counts in the duplicate-listing refusal: it may still execute, or
  be verified later. The routes that list, price or report
  the status of trades refuse with the `trade-events-index` envelope until
  the index is current on both networks. `/v1/trades` and
  `/v1/trades/{id}` serve the stored records, which carry no status, so
  they wait for the trades mirror only.

## Curation committee

Builder's committee routes (the drafts and items of every wallet, another
wallet's collections and items, the curation console's committee list)
admit `BUILDER_ADMIN_ADDRESSES` and the current members of the Polygon
curation committee. The testnet pair schedules no committee walk. Builder
uses the configured admin addresses there. Shop's mainnet `committee` index
(`MARKETPLACE_COMMITTEE_INDEX_ENABLED`) walks the two Committee contracts'
`MemberSet` logs from the first one's deployment into `shop.committee_event`
over the `TRADE_RPC_URLS` 137 entry, else on mainnet `RPC_ENDPOINT_POLYGON`,
once the endpoint answers chain id 137 (tens of rows, appended with the cursor advance in `shop.chain_cursor`,
deleted past a reorg rewind). Builder reads, never writes, both tables by
shared SQL in one statement: `shop.committee_event` (`network`,
`block_number`, `log_index`, `member`, `is_member`) and `shop.chain_cursor`
(`indexer`, `network`, `full_history`, `chain_id`, `quarantined_at`,
`folded_block`, `folded_at`); the cursor must be bound to chain 137 and out
of quarantine (see "Market chain index cursors"). A
wallet is a member when its latest event (block, log index) at or below the
published block (`folded_block`) set it, so rows a pass adds before its
publish wait for it. Published membership stays readable while a pass
runs. When Polygon is in scope, Shop accounts require a fresh committee
fold covering the published sales block and include every indexed member
through that block, including former members. An index that is off, not yet published or
published more than 30 minutes ago (`READY_WINDOW`) admits only the
configured addresses, and a failed read answers 503. Changing a column builder reads updates this section
and the AGENTS.md "Ownership" entry in the same change.

## Third-party registry

The Polygon registry walk uses the `third_party_registry` cursor on `matic`
and appends one version to
`shop.third_party_registry_event` per registry event that changes a known
provider. The state holds the event's provider metadata, linked contracts,
approval, root, resolver, managers and slot counts. A consumption event also
holds its message hash, quantity, signer and curator. Block time and position
identify when the receipt was recorded. Exact decimal strings preserve the
contract's 256-bit counts.

The registry has a small provider catalogue and far more reads than writes.
The primary key supports rewind by block and the provider version index
supports the previous state lookup on write and the latest state lookup on
read. Each version and its receipt commit with the cursor advance. A rewind
deletes the affected suffix, so prior provider state is already available
at the remaining event positions. No TypeScript import writes this table.

Provider queries read the latest state per provider at or below the same
statement's published block. Only then do they filter approval or manager
membership. Rows from a partial, stale, diverged, quarantined or wrong-chain
publication cannot be served. An empty result from a current publication
means there are no matching providers. The index exposes on-chain slot
availability as the maximum less consumed slots. Publish reservations and
submission state still belong to the service that processes the submission.

## Collections index

The Polygon `collections` walk (`MARKETPLACE_COLLECTIONS_INDEX_ENABLED`, enabled
in the mirror posture) reads the `TRADE_RPC_URLS` 137 entry. It walks two kinds
of log from the first factory's start block (15,202,000), on the shared
`shop.chain_cursor` (see "Market chain index cursors"):
- the collection factories' `ProxyCreated`;
- the events of every collection the factories created.

Those events are the items added, rescued and edited, the mints (`Issue`),
the global and item minters and managers, approval, editability,
completion, creatorship and ownership. The walk keeps them in
`shop.collection_event`, appended with each window's cursor advance and
deleted past a rewind. A process that runs the collections mirror
(`MARKETPLACE_MIRRORS_ENABLED=true`) keeps the log alone. One that does not
also projects it into `shop.collections` and `shop.items` (see
"Projection"). Either way, the publication means everything the fold wrote
is complete to `folded_block`.
Account inventory and lifetime creator rankings require the Polygon log
publication. Creatorship transitions are indexed by contract and by both
wallet roles.

**Workload.** Polygon holds about 6,200 collections, 12,000 items and 5.3
million mints, so the log is about 5.3 million rows, nearly all `issue`.
Averaged since the first factory (2021), that is about 3,000 mints a day.
Every mint is kept: an item's supply and its minters' allowances are exact
only from them. Measured on Postgres 17 with one million rows shaped like
the walk's `issue` rows, a row takes 174 bytes in the table and 41 in the
primary key. 5.3 million rows therefore take about 1.1 GB. A window writes its rows
in batches of 1,000 inside the cursor's transaction. The primary key
(network, block, log index) serves the rewind's range delete. The partial
index on `proxy_created` lists the walked collections for the address
filter and the creation reads.

**Address filter.** A window's filter is fixed before its logs are read. A
collection created inside a window emits its `initialize` events right after
the factory's `ProxyCreated`, in the same transaction, so the walked
`ProxyCreated` logs cannot supply the filter for that window.

The filter takes the factories' own lists instead: `collectionsSize()` and
`collections(i)`, read by the gate before each attempt into
`shop.collection_hint`, one row per address, about 6,200 rows that only
grow. That is a superset of what any window can create. A factory's
collections enter the filter from the block the factory was deployed. Each
index is read once at the finalized block and marked final; the indexes
past it are read at the latest block at every refresh until the finalized
block reaches them, so the filter already takes a collection the walk can
reach. A provisional address that a reorg replaced stays as a hint the
filter takes in vain, and the address read at the finalized block is added
beside it. Once every list is final up to the finalized block, a refresh
costs the four `collectionsSize()` reads.

A refresh reads at most 1,000 indexes, in chunks of 200 kept as soon as
they are read. Both lists' indexes below their finalized sizes come first,
and the provisional ones get what is left. While those final indexes are
not read whole, the attempt waits for the next interval; a fresh node's
lists take about seven attempts. Provisional indexes the budget leaves
unread never hold an attempt back; a window that walks one of their
collections still fails.

A size past 1,000,000 fails the refresh before any index is read. The
indexes to read are found from the stored rows, so a refresh's work is
bounded by the table and the budget whatever an endpoint answers. If any
index does not answer, the refresh fails the attempt, so no window walks on
a list that was not read whole. A JSON-RPC batch the endpoint refuses, or
answers only in part (geth answers a batch past its limit with one error),
is halved and sent again; a single call it refuses as a batch goes as a
plain request, and an endpoint that answers batches with nothing gets single
calls. Holes are counted below the finalized size the endpoint answers, so a
final row stored past it does not hide one. A window that walks a `ProxyCreated` of an address outside its
filter fails before it commits, and the next attempt reads the lists again.
Every pass reads the hints from the table again, alone or shared. A
hint is only a filter: a collection exists for the index once its
`ProxyCreated` is walked. The gate first checks that the endpoint serves
chain 137.

**Creation state.** `initialize` sets a collection's name, symbol, creator,
approval and editability without an event. The gate reads them once per
walked collection into `shop.collection_seed`:
- only collections whose `ProxyCreated` the walk has walked, oldest first;
- at most 500 collections per attempt, ten JSON-RPC batches of 100 calls
  for each 200 collections;
- at the walk's own cursor block, named by the hash the walk stored for it
  (EIP-1898 `{"blockHash", "requireCanonical": true}`), recorded in
  `read_block` and `read_hash`.

The cursor and the walk's owner generation and epoch are read in one
snapshot, and the write checks under the reads lock that they did not move.
So a read is of the walked branch whatever any header says, and it is
written proven (`verified`). A rewind below it deletes it in the cursor's
own transaction. An endpoint that no longer holds that block canonical, or
does not serve its state, refuses the read; a read that fails is logged and
does not stop the walk, and the collection waits for a later attempt.

Whatever folds the log takes a value from the last walked event that sets
it. It takes a creation read only when it is verified, no event sets the
value, and `read_block` is at or below the block it folds to; then the value
has not changed since creation. Rows written before reads were proven (no
hash) are never taken, and the collection's first proven read replaces
them.

**Factory list reads ahead of the walk.** The gate reads the lists at the
finalized block, usually past the walk's cursor, where a reorg is invisible
to the walk. Every read there names the block by hash (EIP-1898), in batches
and in the single requests they fall back to alike: what is read and the
hash it is stored with are one block's, and an endpoint that left that block
refuses the read. Only the provisional reads go to `latest`. A final row
keeps that height and hash, and until it is `verified`, before each attempt:
- a row with no hash (written by an index published before hashes were
  kept) needs no read where a walk with its full history holds its exact
  slot (below): it is proven there, with the creation's block as its height,
  so a published walk does not wait for its lists to be read again. Any
  other is undone without a read: it becomes provisional again, to be read
  again;
- at or below the cursor of a walk with its full history, the walk's own log
  decides, slot by slot, with no header asked. A factory's list grows only by
  the push after each `ProxyCreated` it emits (both factories'
  `createCollection`), so slot i is the factory's i-th walked `ProxyCreated`
  in (block, log index) order. That one naming the row's address, at or
  before that height, proves the row (`verified`; from then a rewind below it
  undoes it). Anything else is undone, and the slot is read again: with
  CREATE2 a collection keeps its address on a branch that created it in
  another order, so the address alone proves nothing;
- otherwise (ahead of the walk, or on a walk without its full history,
  whose log lacks the older collections) the endpoint's hash at that height
  is asked. Another hash undoes the row; the same one leaves it as it is. A
  header never proves a row: two replies may come from two branches.

Undoing runs under the reads lock (see Projection), exclusive, and raises
the walk's epoch, so a read in flight from the left branch is not written;
nothing is written if a rewind or a new owner moved the walk since the
check began. Headers go in batches of 100, then one request each for what a
batch did not answer whole, or for all of them on an endpoint that takes no
batches; a header the endpoint does not serve fails the attempt. One
attempt asks at most 100 heights, going on from where the last stopped
(`shop.collection_walk.checked_through`), so every height is asked within a
few attempts. They are the heights of attempts that read a list past the
walk's cursor: a handful once the walk is at the head.

So a collection replaced at a list index above a lagging cursor is read
again by the next attempt, and the window that walks it commits without a
rebuild.

**Decoding.** Every factory collection shares one ABI for these topics. A
log of a walked topic that does not decode fails its window instead of
being skipped. A `ProxyCreated` from any contract other than the two
factories fails it too. Strings are read as lossy UTF-8, with NUL removed.

**Projection.** A network the operator hands the index
(`MARKETPLACE_COLLECTIONS_INDEX_OWNS`, `matic` or empty, independent of
`MARKETPLACE_MIRRORS_ENABLED`) is projected. When a pass closes, the fold
writes Polygon's catalogue rows from the log and the creation reads, in one
transaction fenced by the walk's owner. Without that setting the walk keeps
its log alone, whatever the mirror switch says. `shop.collection_projection`
holds one row per network:
- `owned_at` is set in the transaction of the first projection. From then
  the fold is the only writer of the network's rows in both tables. A
  hand-back clears it.
- `projected_through` is the block the rows stand at.
- `pending` counts the walked collections without a row at that block,
  waiting for their creation read. The catalogue is complete only at 0.
- `generation` grows with every projection and hand-back, so two reads under
  the same generation read the same rows.

The mirror refuses to write a network the fold owns, and its job skips that
network: no page is read. It keeps walking the others. Both take the
network's advisory lock (`pg_advisory_xact_lock(0x636f6c6c, <chain id>)`)
around the ownership check and the write: the mirror shared, for each page,
and the fold exclusive. So a page that read the network as unowned commits
before the projection that takes it, and none commits after.

A projection folds the events past `projected_through` onto the stored
rows. It folds a collection again from its creation in three cases:
- it has no row yet (created since, or waiting for its creation read);
- an event names one of its items that has no row (metadata no item type
  takes);
- the walk rewound. A rewind clears `projected_through`, and the next fold
  rebuilds every row of the network in one transaction, replacing the rows
  it does not produce, the mirror's included.

A rewind also undoes the gate's reads taken past its point, so a read of a
branch the chain left is read again on the new one:
- it deletes the creation reads taken past it;
- it makes the factory list rows read at a finalized block past it, or at an
  unrecorded block, provisional again;
- it raises the walk's epoch (`shop.collection_walk`).

It holds the reads lock (`pg_advisory_xact_lock(0x72656164, <chain id>)`)
exclusive. A write of those reads holds it shared, and checks that the walk's
owner generation and epoch did not move since its reads began. So a read
taken before a rewind is never written after it, and one written while it
runs is undone by it. An operator's rebuild runs the same rewind from the
start block, in its own transaction, so it undoes them all and raises the
epoch too.

A collection waits, with no row and without holding the publication back,
in either case:
- until its creation read exists (a proven one, see Creation state);
- while that read was taken past the block and no walked event sets the
  value it gives.

The fold from the creation reads the collection's events other than mints
through the partial index on them, a few hundred thousand rows. Mints come
in three forms:
- the mints of item minters with a finite allowance, which it replays;
- each item's highest issued id;
- the block time of the item's newest mint by the CollectionStore,
  aggregated from the store's issues.

A rebuild aggregates the whole log once. A resume reads only the events past
`projected_through`, by the primary key.

The rows follow the contract where the collections subgraph differs:
- global and item minters and managers are sets; an account is listed once,
  while its newest grant set it, in address order;
- the store sells an item while it is a global minter or holds an allowance
  for the item, and an item it can sell from its addition is listed from
  then (`first_listed_at`);
- each mint an item minter calls spends one of a finite allowance, unless it
  called while it was the creator or a global minter, as `_issueToken` does.
  `shop.items.minters` and `minter_allowances` hold the item minters with an
  allowance left, and what they hold;
- an item's supply is its highest issued id.

Item metadata follows the subgraph's format: `w`, `sw` and `e` items,
wearables with a known category, emotes with an unknown category as `dance`,
and an emote's social outcome as `simple_outcome`, `multiple_outcome` or
`random_outcome`. Search text folds ASCII letters alone, as the subgraph's
helper does. `sold_at` is the newest primary sale only; secondary sales come
with the sales index.

The catalogue routes still decide readiness from the mirror's passes
(`shop.mirror_state`), so no network is taken from the mirror while they do.

**Switching a network from the mirror.** Run this after the catalogue routes
follow the index and the NFT and sales walks take their contracts from it.
Deploys are manual, one process at a time:

1. Run the enabled walk (`MARKETPLACE_COLLECTIONS_INDEX_ENABLED=true` in the mirror posture) on the
   process that runs the mirror, and wait until the `collections` cursor on
   `matic` publishes. That is the catch-up, from block 15,202,000. The
   mirror keeps serving meanwhile.
2. Stop that process, set `MARKETPLACE_COLLECTIONS_INDEX_OWNS=matic`, and
   start it again. Two processes must never write the catalogue under
   different settings. The mirror switch stays as it was: the mirror keeps
   walking Ethereum. The next pass's fold rebuilds Polygon's rows from the log
   and takes the network.
3. Check `SELECT owned_at, projected_through, pending, generation FROM
   shop.collection_projection`.

**Handing a network back to the mirror.**
1. Stop the index process, remove the network from
   `MARKETPLACE_COLLECTIONS_INDEX_OWNS`, and start it again. Its fold keeps
   the log alone from then.
2. Run this transaction. It takes the exclusive catalogue lock, and removes
   the network's rows, the mirror's pass markers and the ownership together:

   ```sql
   BEGIN;
   SELECT pg_advisory_xact_lock(1668246636, 137);
   DELETE FROM shop.items WHERE network = 'matic' AND EXISTS (SELECT 1 FROM shop.collection_projection
          WHERE network = 'matic' AND owned_at IS NOT NULL);
   DELETE FROM shop.collections WHERE network = 'matic' AND EXISTS (SELECT 1 FROM shop.collection_projection
          WHERE network = 'matic' AND owned_at IS NOT NULL);
   DELETE FROM shop.mirror_state WHERE network = 'matic' AND mirror IN ('collections', 'items')
      AND EXISTS (SELECT 1 FROM shop.collection_projection WHERE network = 'matic' AND owned_at IS NOT NULL);
   UPDATE shop.collection_projection SET owned_at = NULL, projected_through = NULL, pending = 0,
          generation = generation + 1, updated_at = timezone('UTC', now())
    WHERE network = 'matic' AND owned_at IS NOT NULL;
   COMMIT;
   ```

   Every statement acts only while the index still owns the network, so
   running the transaction again (after a commit that went unconfirmed)
   changes nothing the mirror wrote since.

Every read of the network then answers the unavailable envelope until the
mirror's next complete collections and items passes, which rewrite it with
the mirror's rows alone (`collections_projection::hand_back` runs the same
statements).

## Auth API relay state workload

`/auth-api` (`services/catalyst/explore/src/api/auth_api/`) is the sign-in
relay: a client posts a signing request, a wallet page recovers it by id and
posts the outcome, and the client polls for it; a signed identity is parked
for one pickup by a device. The gateway spreads `/auth-api` across the API
replicas round-robin without affinity, so the state lives in three
`explore_api` tables (`schemas/11_explore_api.sql`) every replica reads and
writes through the explore pool, as the upstream auth server keeps it in Redis.

Socket.IO sessions hold the requests they create on the replica that owns
the connection. The gateway consistently routes Engine.IO polling and
upgrades by client address. While the replica has bound requests, it checks
their live validation and outcome state through one primary-key batch query
per 500 ms pass. The socket and HTTP poll share a row-locked outcome claim.
If the socket cannot queue the event, the claim rolls back for HTTP pickup.

Workload: a sign-in is one create, one or two recover reads, a validation mark
at most, one outcome and a poll about once a second until the outcome lands;
an identity is one create and one pickup. Rows are small (a challenge of a
few hundred bytes; an outcome or identity of 1-2 KB with its auth chain) and
short-lived. The caps bound the live set: at most 10,000 undelivered
unexpired requests (100 per creating network) and 10,000 unexpired
identities (100 per network). No production rate is measured.

- `auth_requests`: one row per request, keyed by its UUID. `status` moves
  only forward, `pending` -> `signed` -> `fulfilled`; `outcome` is set exactly
  while it is `signed` (a `CHECK`). The outcome is stored by
  `UPDATE ... WHERE status = 'pending'`, so of two answers the second finds it
  signed (400). It is delivered once: the poll's `UPDATE ... FROM (SELECT ...
  WHERE status = 'signed' FOR UPDATE) RETURNING` moves it to `fulfilled` and
  clears `outcome`, `params` and `challenge`, so a concurrent poller on any
  replica waits on the row, finds it fulfilled and answers 410. A request
  expires 5 minutes after its creation. Reads answer 404 after expiry, and
  the periodic sweep deletes expired rows.
- `auth_identities`: one row per parked identity, keyed by its UUID, holding
  the identity document with its ephemeral private key for at most an hour.
  The key is stored in clear, as the upstream auth server keeps it in Redis:
  it is never logged, but the row reaches the WAL, the streaming replicas and
  the backups, which keep it after the row is deleted for as long as they
  are retained. A copy of the key signs for the identity's account until the
  expiration in its auth chain.
  A deep-link create for a client that holds a verifier (Creator Hub) binds
  it to the client's `authRequestId` (`auth_request_id`, lowercase), which
  the client derives from the verifier it presents on pickup
  (`x-identity-verifier`); other identities stay unbound and keep the IP
  check. The pickup locks the row (`SELECT ... FOR UPDATE`), decides, and
  deletes it in the transaction that writes its tombstone when it is served,
  expired or refused for its IP, so exactly one pickup gets it; a concurrent
  one waits on the row, finds it gone and reads the tombstone. A bound
  identity is served only to its verifier, from any network; a pickup with a
  wrong or missing one (401) leaves the row parked.
- `auth_identity_tombstones`: why an identity is gone (`consumed`, `expired`,
  `ip_mismatch`, or NULL while parked and once evicted by the sweep or an
  operator's revoke), for the 404 bodies, kept 14 days and at most 50,000
  rows: a create past that stores none, and a pickup sets the reason of the
  row its create stored or, when there is none, inserts one only under the
  same cap. Pickups take no lock, so concurrent ones can pass the count
  together; the overshoot is at most the pickups in flight.

Caps: once its chain passes the checks that need no RPC, a request create
counts its network's live rows without any lock and refuses one over the
per-network cap (429) there; an identity post does the same only when a
validator is configured and its chains need a contract-wallet call. When a
contract-wallet call is coming, the network's and the replica's contract-wallet
budgets are checked first, so a request with none left gets its 429 without the
count. A flood from one network thus neither reaches the contract-wallet RPC
nor queues on the lock. Then a create takes `pg_advisory_xact_lock` on one key
per table (`hashtextextended('explore_api.auth_requests:admission', 0)` and the
identities' twin), counts the live rows and those of its network in one pass
and inserts in the same transaction, so concurrent creates on every replica
admit one at a time and never overshoot. A full request table first drops
expired requests, then the oldest delivered ones, just enough to admit
one more. The count scans the table, bounded by the caps to about 10,000 rows;
sign-in creates are rare enough that serializing them on the lock costs nothing
measurable.

Magic account deletion inserts one short-lived row per verified request into
`explore_api.magic_deletion_attempts`. A digest of the wallet address and Magic
token ID is the unique key. An advisory transaction lock serializes the
one-use claim across replicas. The default hourly caps allow 3 attempts per
wallet, 20 per network and 120 globally. A network is one IPv4 address or an
IPv6 /48. Requests with no trusted source share a network bucket. A refused
request writes nothing and counts in
`explore_magic_deletion_rate_limited_total` by the cap it hit. Admission counts the preceding hour
through the requested-at index, which also serves the one-day expiry sweep.
The row stores the wallet and source digests and the outcome category. A row
still marked requested can represent an interrupted operation. The table
holds neither DID tokens nor Magic response payloads. The relay sweeper
also prunes these rows when Magic deletion is configured.

Contract-wallet budget: contract-wallet calls (`isValidSignature`, including
those of the signed-fetch chain on `POST /identities`) are budgeted in process
memory (no table), since a forged chain stores no row for the caps to count: 20
calls per network per minute, 200 shared by requests with no network, 200
shared by networks past 4,096 tracked ones, and 1,000 per replica. The calls
behind a request, outcome or identity that is stored are given back; failed
calls are not. A network past its budget gets a 429 before the RPC. The budget
keys an IPv6 caller by its /48 (the pending-request cap keeps its /64), so a
whole /48 shares one 20-call budget: on a shared carrier or hosting range one
sender can spend it and bring everyone in that range a 429. About 50 networks
of forged calls together can spend the replica's 1,000 in a window: every
contract-wallet sign-in on that replica then gets a 429 ("on this node") until
the window ends, while EOA chains are unaffected; the replica logs a warning
once per window when that happens. During an RPC outage, after 20 failed calls
(about 10 sign-ins) a network sees that 429 instead of the 503 for up to a
minute. Below this budget, the node's shared validator applies its own to every
contract-wallet check that reaches the RPC on any route
(`shared::eip1654_budget`, also in memory): 120 RPC calls per client source a
minute (an IPv6 caller by its /64) and 480 per IPv6 /48, each admitted before it
is sent, with a 429 past it; a cached acceptance spends nothing. An
`isValidSignature` check is one call; a verification-block lookup spends one
per call it makes.

Expiry compares the stored instant with the handling replica's clock, so
replicas need synchronized clocks (NTP). The periodic
`auth-api-identity-sweep` job runs on every replica every 30 seconds. It
deletes expired requests, expired identities, tombstones older than 14
days and, when Magic deletion is configured, Magic deletion attempts older
than a day. Each row is deleted once across replicas. The expiry indexes
(`auth_requests_expiration_idx`, `auth_identities_expiration_idx`,
`auth_identity_tombstones_created_idx`,
`magic_deletion_attempts_requested_idx`) serve those deletes. The Magic
admission's hourly count reads that last index too. Every other read and
write is by primary key. Nothing references these rows and they
reference nothing: no FK. The bootstrap defines the tables; an existing
database gains them from the object pass, so no step exists. A restart of
every replica keeps the relay's state.

## Estate data documents workload

`land_authz.estate_data_event` (`schemas/33_land_authz.sql`) holds the
Estate registry's data documents, so the map metadata of a dissolved estate
(a minted Estate token holding no LAND) carries the name and description
the atlas reads from the marketplace subgraph.

- **Input:** the Estate registry's `CreateEstate` and `Update` logs, fetched
  by the land-authz pass in the same `eth_getLogs` as the rights topics.
  One log per estate mint and per metadata edit. The LAND registry emits
  `Update` under the same topic for parcel data; those logs are fetched and
  dropped before timestamp hydration. Neither volume is measured; the
  extra LAND logs count toward the endpoints' per-response log cap and may
  narrow the ranges a pass reads.
- **Row:** one per log whose document parses as version 0, with the
  subgraph's `buildData` over graph-ts `parseCSV`
  (`land-authz/src/events.rs`). `name` and `description` are NULL when the
  document omits them. A document that does not parse writes no row, as
  the subgraph keeps no data for it. Key `(block_number, log_index)`, the
  log's own position, as `authz_event`.
- **Write:** appended in the transaction that writes the range's
  `authz_event` rows and advances the cursor, with `ON CONFLICT DO
  NOTHING`. A decode failure never fails the pass: the row is skipped.
- **Read:** `LandAuthzState::dissolved_estate`, on the published snapshot
  (`ready_snapshot`): one `token_right` primary-key probe, one
  `estate_land_estate_idx` probe and the latest row by
  `estate_data_event_estate_idx`. It runs only for an estate the tile grid
  does not hold (`/v2/estates/{id}`, `/v2/contracts/{estate}/tokens/{id}`).
- **Invariants:** an estate's latest row names it. A later document that
  omits a field leaves that field empty; the subgraph's merge may keep the
  earlier value there instead (unverified). A reorg rewind deletes the rows
  past its block in the transaction that deletes the log's. A registry
  switch keeps the rows of the old registry, which reads never match
  (`token_address`). No FK: rows reference no table and nothing references
  them.

The table and its index are new objects: the object pass creates them on a
stamped database, so no step exists. On a node whose index already passed
the Estate registry's history, the table holds only the documents indexed
after the upgrade, and a dissolved estate's name and description read empty
until the operator sets `LAND_AUTHZ_REPLAY_FROM_BLOCK` to the Estate
registry's deployment block (mainnet 6,236,547, the marketplace squid's
`EstateProxy` start block) and unsets it once the index publishes again.
That replay re-reads both registries from there; until it publishes,
readers use the subgraph fallbacks and Genesis collaborator grants fail
closed. Its duration is not measured. Completeness check: `SELECT
min(block_number) FROM land_authz.estate_data_event` is at or below the
first `CreateEstate` block.

## Map grid build workload

`explore_api.map_grid` (`schemas/11_explore_api.sql`) holds the tile grid
behind the explorer map routes (`/api/v{1,2}/tiles`, `/api/v2/tiles/info`,
`map.png`, the minimaps and the parcel and estate metadata). The node
builds it from its own indexes.

- **Job:** `explorer-map-grid` (`explore/src/api/map/mod.rs`, `watch`),
  leased, started once the land-authz index and Shop's reader are attached,
  whatever `EXPLORER_MIRRORS_ENABLED` says. It builds when its inputs
  change, not on a timer: every 5 s it reads their marks (`Watcher`), with
  the land-authz readiness and the clock, in one REPEATABLE READ, READ ONLY
  transaction, so an observation that found the index published never
  carries a clock from after it stopped being. A pass (`run_pass`) runs
  when they moved since the ones the last pass read in its own snapshot,
  at its clock, when a change held back by the publication floor may
  publish, when a failed pass's retry is due (1 minute, doubling to 5), when another
  publisher stored a revision during the last pass, or after 10 minutes
  without one (a safety net). Moved marks wait while not every source is
  ready (a pass could not build; the marks move again once they are, and
  the safety pass still runs and records what it waits for), while a change
  is held back (the pass at the floor's end reads every input) and while a
  failed pass waits for its retry, unless every source turned ready
  meanwhile; marks that cannot be read move nothing. So a failing read or
  pass backs off instead of building every 5 s, and a source briefly
  unready (a LAND walk between its commit and its fold) fails no pass. Because later marks are compared with the
  pass's own, a change that lands after its snapshot moves them even when
  a later change reverts it (a rental listed, then closed). A pass that
  could not read its marks vouches for nothing: the next readable marks
  run a pass before any check, and a second such pass in a row backs off
  as a failed one does, as does a pass that finds its body current but
  cannot record the check. Only marks equal to the pass's record its
  revision checked again.
  Each mark is a durable counter raised in the transaction that rewrites
  rows in place, a position in an append-only log, or a digest of the very
  rows the build reads, so equal marks mean equal inputs:
  - The land-authz marks (`LAND_MARKS_SQL`): the cursor's readiness, its
    `rewrites` count and the newest folded LAND/Estate rights event and
    estate data document, each a backward scan of its primary key from the
    published block. `land_authz.index_cursor.rewrites` is raised in the
    statement of a reorg rewind, a replay rewind and the legacy block-time
    repair (which can run while the index is published); between two
    raises both logs only grow past the published block and the rights are
    their fold, so a rewind that refolds to the same positions with other
    facts still moves the marks. An event past the published block moves
    nothing.
  - Shop's marks (`land_map::land_market_marks`, one statement on the
    same connection and database clock): the nft, trade-events and trades
    readiness; the Ethereum nft cursor's `reorg_rewinds` and `rebuilds`
    (`shop.chain_cursor`, raised in the transaction of a rewind, of the
    rebuild that clears the projection and of the estate repair's refold);
    the two registries' live orders at that clock (expiry compared in
    seconds rounded half up, as the build compares it) and their newest order
    event; their names' count and newest document; and the SHA-256 of the
    open MANA trades and of the open rental listings exactly as the build
    reads them at the clock (the trades after the trade-events verdicts).
    The counts and digests are taken at the clock, so an expiry moves them
    too. Digesting every order and name instead would cost about 170 ms a
    read on the synthetic database below; the statement takes 27 ms.
  - With nothing moving and every source ready, the job records the
    revision the last pass left current checked again: an `UPDATE` of
    `explore_api.map_grid.checked_at` guarded on that revision's stamp and
    `etag`, on the lease's session, at most once a minute. A check that
    finds another revision stored records nothing and runs a pass at once.
    A pass that stores a grid or finds its body equal to the stored one
    sets `checked_at` too. `checked_at` is `/v2/tiles/info`'s `checkedAt`,
    the time the map's age label reads (beside `serverNow`, the process's
    clock when it answers, which the label measures it against: a
    cross-origin client may not read the `Date` header), and stops moving while a source is
    down or the build is not running. The pass marker, `map-grid` in
    `explore_api.mirror_passes` (`GET /mirrors`), records build passes
    only; a check is not one.
  Every process also runs `explorer-map-grid-reload` every 10 s (`follow`):
  one primary-key read of the row's stamp and `checked_at`, a reload only
  when another process stored a newer grid, `checkedAt` only for the
  revision it serves, and one read of the pass marker for what the build
  waits for.
- **One snapshot:** every read below runs in one REPEATABLE READ, READ ONLY
  transaction, the one the land-authz index opens once it finds itself
  published for the configured registries
  (`indexer::ready_snapshot`; a database error fails the pass as itself,
  not as an unpublished index). The build's clock is that snapshot's
  `now()`: every comparison the build makes (the readiness windows, expiry,
  the stamp, the publication floor) uses that one PostgreSQL transaction
  clock. Shop's readiness checks and rows run
  on the same connection, which `serve` hands to Shop through
  `src/serve_adapters.rs` (Shop sits above Explore). The LAND facts, the
  index cursors and Shop's rows are therefore one state of the database: a
  rewind committed during the build, a withdrawn publication or deleted
  rows, is invisible to it, and the next pass sees it whole.
- **LAND reads** (`explore/src/api/map/build.rs`): every LAND `token_right`
  row with coordinates, its `estate_land` row and the `authz_event` row its
  `(updated_block, updated_log)` names, by primary key, for the time of its
  last rights event. Every Estate `token_right` row that holds LAND, with
  the same event join and the name of its latest `estate_data_event` at or
  before the snapshot cursor's `folded_block` (`estate_data_event_estate_idx`,
  one probe per estate), the bound the marks read it with: a document the
  walk committed past the fold shows once it is folded.
- **Shop reads** (`shop/src/market/land_map.rs`): the Ethereum
  `shop.nft_order` rows of the LAND and Estate registries, whatever their
  status (primary-key range). The `shop.land_data` rows of both registries,
  with the `shop.nft_event` row their `(updated_block, updated_log)` names,
  by primary key. The open `public_nft_order` trades that sell a token of
  either registry (`catalog::unified_trades_cte_of` scoped to the two
  contracts, `idx_trade_assets_sent_contract`), priced in MANA: received
  as an ERC-20 (asset type 1) whose token is the chain's MANA
  (`listings::mana_token`); another token or a USD-pegged price is not a
  MANA amount. The open, unexpired
  `signatures.rentals` rows of both registries signed for the grid's chain
  and its Rentals contract, with their periods.
- **Readiness:** the land-authz index published for the configured
  registries; the grid's chain being Ethereum mainnet, the chain this grid reader covers; the nft index published on Ethereum and bound to chain 1
  (`shop.chain_cursor`, `chain_walk::ready_sql`); and `shop.trades`
  readable (a recent complete trades-mirror pass read in the snapshot, or
  local authority); and the trade-events index current on Ethereum, whose
  verdicts (`shop.trade_chain_state`) close a sold or cancelled trade, as in
  the Shop's own listings, which fail closed the same way. A source that is not ready fails
  the pass, naming it in the marker's `last_error`, and nothing is built.
  The held grid keeps serving. Until a grid exists, every map route answers
  the unavailable envelope (503, `Retry-After: 60`, `no-store`, no
  validators, even to a conditional request), naming the source the build
  recorded, or the grid itself (`map-grid`, no `configure`) before any
  build has.
- **Write:** the stored row is the grid's revision. `etag` holds the hex
  SHA-256 of the body a build stored (the canonical JSON, tiles in key
  order), and `last_updated_at` its stamp, which only grows. A pass reads
  the row (or its absence) in its input snapshot, as the revision its
  inputs follow, and compares its body with that revision, not with what
  its process holds. An equal body writes nothing, and a process holding an
  older grid reloads the row. A different body waits until that revision
  was built 1 minute earlier (`fetched_at`, `MIN_PUBLISH_INTERVAL`), so a
  token relisted many times a minute writes at most one 4.4 MB row, and
  causes one reload in every process, a minute; the job runs the pass that
  publishes it when the minute is up. Then it is gzipped and written with an `UPDATE` guarded on that
  revision's stamp and `etag` (an `INSERT ...
  ON CONFLICT DO NOTHING` when it saw none), stamped with the current time
  in milliseconds and at least one second past it. A leased build runs that
  write on its lease's own session (`LeaseControl::with_connection`): it
  commits while the process holds the lease or not at all, and a lease lost
  while the statement waits takes the statement with it. If another publisher
  stored a revision after the snapshot began, the write stores nothing and
  the build is discarded: a pass never overwrites a revision published
  since its snapshot. A process serves a grid only if its stamp
  is newer than the one it holds, decided under its grid lock, so a slow
  reload never brings back an older revision. An order or rental that
  expires moves the marks at its expiry; the pass then publishes the change
  under the same floor. A PNG answer takes one grid
  for its validators, cache key and body, and for an estate's
  dissolved-estate redirect.
- **Rules:** atlas-server's `buildTile`, `addSpecialTiles`,
  `getEstateFlags` and `applyTrades`, with its roads and plazas vendored
  (`special_tiles.json`). The rules and the known differences are in the
  `build.rs` module docs. An order whose seller no longer owns the token
  shows no price, while the atlas still prices it until it expires: 19 of
  the 412 tiles the atlas priced by a legacy order on 2026-11-24. Bids do
  not move `updatedAt`. Rental listings are this node's own, and a listing
  is not checked against the token's current owner: the
  `rental-listing-status` job cancels one whose asset left its lessor.
  While a source is not ready, the held grid keeps its prices and listings,
  including ones that expire meanwhile, until a pass builds again.
- **Volume and cost:** mainnet holds 92,598 LAND, 65,086 of them in about
  2,700 estates. On a synthetic database of that size (1.02M rights
  events, 30,866 orders, 69,448 names among 1.07M `nft_event` rows, 52,000
  trades), measured on a laptop with Postgres 17 and a release build: the
  LAND snapshot read takes 0.47 s, the estates 16 ms, the orders 10 ms, the
  names 34 ms and the trades 16 ms. Composing the grid takes 0.14 s,
  serializing it 0.19 s (a 36.5 MB body) and gzip 0.04 s (4.4 MB stored).
  A published revision then costs the guarded write (60 to 85 ms, 4.7 to
  4.8 MB of WAL), a reload in every process (70 to 93 ms to fetch, inflate
  and parse, 50 to 130 MB of transient memory) and, for every map client
  that fetches `/v2/tiles` again, 36.5 MB (the node does not compress it).
  The 1-minute floor allows at most 60 revisions an hour (about 290 MB of
  WAL and 60 reloads per process). The live atlas published no revision in 80 minutes on
  2026-11-24 (its `landVersion` and `overlayVersion` held), so the floor
  bounds bursts, not the usual rate. Watching costs the marks every 5 s
  (about 7 ms for land-authz and 27 ms for Shop, under 25 s of database
  time an hour) and at most 60 single-row `checked_at` updates an hour,
  which leave the stored body untouched.
- **Deploy:** one additive step, `0066_map_grid_change_marks` (the
  `rewrites`, `rebuilds` and `checked_at` columns). Stop every process of a binary that runs
  the `explorer-map-mirror` job before this one's first build: that job
  rewrites the row's stamp without a guard. An environment that still sets
  `EXPLORER_MAP_SOURCE_URL` or `EXPLORER_MAP_MIN_REFRESH_SECS` starts, as
  the binary and the posture checks read neither. A node without the
  land-authz index, the nft index (`MARKETPLACE_NFT_INDEX_ENABLED`, mainnet
  only) or a trades source never builds a grid, and its map routes answer
  the envelope.

## Place removal feed

`explore.place_removal_pending` (`schemas/10_explore.sql`) is a sparse outbox
of catalogue removals. Explore's trigger on `explore.place` and
`explore.place_world_local` inserts one row per transition to removed (opt-out
is not a removal), snapshotting the place context, and keeps one lifecycle
bit per place in `explore.place_removal_state`; it is the only writer of new
rows. The comms gatekeeper (`services/catalyst/comms/src/gatekeeper/removed_places.rs`)
consumes it by shared SQL: at most 100 due rows a pass on
`place_removal_due_idx`, `owner_address` filled once when it was empty,
`cleaned_at` set in the same transaction as its destructive cleanup so a retry
cannot replay it, `attempts` and `retry_at` advanced with capped backoff, and
a cleaned row deleted, and the place's `place_removal_state` row with it once
neither catalogue holds the place and no pending row for it remains. Its context lookups read
`explore.place_indexed`, joined to `content.active_pointers` and
`content.deployments` for shared-scene rooms, by primary key or world name.
Its cleanup ends the stream keys of the removed place ("Scene stream keys").
Neither table has an FK: queue rows outlive catalogue deletion.

Workload and invariants for removal notices: one immutable recipient set per
live removal, with a durable `recipient_cursor` advanced after each Social
acceptance. In the catalogue removal transaction Explore freezes, for a Genesis
place, the valid active admin wallets of its base parcel into `recipients`. For a
World it freezes the exact scene place keys into `admin_keys` and each active admin
there with its original granter into `admin_grants` (a JSON array of
`[admin, granter]` pairs); World admins are not saved as recipients because
maintenance must first check that the granter still holds the world-wide
authority. Its plain MVCC read of `social.scene_admins` (`place_id`,
`admin_address`, `added_by`, `active`) takes no Social row locks and performs no
Social writes. A re-grant before the first maintenance pass overwrites
`added_by`, `added_at` and `removed_at` on the row; the frozen wallets and pairs
keep the evidence. Safety cleanup also adds the pairs of the World grants it ends
to `admin_grants`, in its own transaction, before notification enrichment runs.
Enrichment then reads the owner, the permission surfaces and the grants (live,
ended by this removal, and frozen), and saves the complete set in one write.
Missing external authority leaves `recipients_resolved=false` with the frozen
evidence unchanged; a saved owner alone is never a complete set.
Retries reuse the complete set and original event time. Initial historical
feed seeding sets `notify=false`, retaining cleanup without sending old notices.

The catalogue legs keep `removal_admin_keys` as exact local evidence. Before
final scene retirement, Explore snapshots currently active identity keys when
no backing `worlds.world_scenes` remain. Removed identities from older
lifetimes are excluded. The removal trigger freezes their union with current
active keys into `place_removal_pending.admin_keys`. An actual scene insert
clears only catalogue evidence, with ordered active-identity locks before
catalogue writes; importing an identity alone cannot clear it. Queue rows
survive re-adds. The existing world-row lock serializes publication; imports
lock affected worlds and identities in order and retire only saved bindings
whose backing scenes are gone. No grant history is inferred from parcel overlap.

## Scene admin grants

`social.scene_admins` (`schemas/40_social.sql`) holds the explicit admins of a
scene, one row for each `(place_id, admin_address)`. `place_id` is the place of
the scene, read from the active deployment by the room resolver
(`ActiveDeployment::place_key`): the `explore.world_scene_place` identity of a
world scene, the canonical base parcel of a Genesis scene (its declared base,
read at `entity_metadata -> 'v'`, when the scene holds that parcel; otherwise
its first parcel). These are the keys that scene storage uses.

Workload: one primary-key read for each scene-signed gatekeeper request of a
wallet that is not the owner (`Policy::is_admin`: `/scene-admin`, `/scene-bans`,
`/scene-stream-access`, `/cast/*`, the stream key recheck), and one read of a
place's rows (tens at most) for `GET /scene-admin`, for a room metadata
refresh and for each streaming notice (a Genesis place's base parcel, the
place of a world key's scene or, for a world removal, of each active scene,
with the grants a place removal ended at its `removed_at` on any scene of
the world). Writes are rare:
one row for `POST` or `DELETE /scene-admin`, and the place removal cleanup.

Invariants:

- A redeploy writes no row. A redeploy of the same place (a world scene whose
  footprint overlaps exactly one scene of its world, a Genesis scene on the
  same base parcel) keeps every grant. A different place has none.
- An owner change writes no row. Ownership is read when a request arrives.
- The cleanup of a removed place (above) revokes the grants made before the
  removal (`added_at`; a cleanup that runs late keeps the later ones): those
  on the places of the active scenes of a world and its queued exact retired
  keys (`explore.world_scene_place`
  by `world_name` on `world_scene_place_deployment_idx`; the id of a world
  place is the world name or an imported id, never the key of a grant), or
  those on the base parcel of a Genesis place when no enabled `explore.place`
  row (`world`, `disabled`, `base_position`) has that base. A retired
  world-scene identity is not bound to a deployment again, so no request
  reads its grants.
- The release script `schemas/releases/scene-admin-place-keys.sql` moves the
  grants of active deployments (a world scene's active identity, a Genesis
  deployment that is not deleted) from the entity id to the place. Stop all old
  API replicas before `catalyst schema migrate`, apply the script with the
  administrative database role, then start the updated binaries: an old
  binary reads and writes grants by entity id. `catalyst serve` refuses
  startup while a grant of an active deployment has the entity key
  (`ensure_scene_admin_place_release`). A binary-only rollback is not
  supported after the move; restore the matching backup. An entity that is
  active in several places (two worlds, a world and Genesis) gives each place
  the grant. For one place and wallet the script keeps an active grant before
  a revoked one, then the row with the newest change
  (`COALESCE(removed_at, added_at)`), also against a row the place already
  has. It removes each entity row that it moved. It can run again: it moves
  only rows that have an entity key, and never changes the key of a row that
  is on a parcel or a world-scene identity.
- A Genesis grant on its scene's least parcel, while the scene's place is
  another declared base, is read by no request of that scene, and no script
  moves it: the same row can be a grant made for an earlier scene whose base
  was that parcel and that the place removal cleanup has not revoked, which
  this scene does not inherit. A database that holds
  Genesis grants on parcel keys when it takes the release can hold such
  rows. The operator lists them with the check in
  `infra/containers/README.md` "Scene admin upgrade" (active Genesis
  deployments whose `social.scene_base_parcel` over `{v,scene,base}` differs
  from the one over no declared base, joined to the active grants on the
  latter) and sets `place_id` to the place on each row made for the listed
  scene.
- A row keeps the entity key when its entity is not deployed, or has no
  parcel that parses. A world scene with no active identity reads its grants
  by entity id, so a retired world scene still reads such a row inside its
  eviction window (`SCENE_EVICTION_TTL_MS`). No job deletes these rows: after
  that window no request reads them and they stay (tens of rows at most).
- `social.scene_base_parcel(parcels, declared_base)` is the one SQL copy of
  the effective base parcel that the room resolver computes
  (`ParcelSet::effective_base`). The release script and the import loads of
  scene admins and scene bans call it; `the_sql_base_parcel_is_the_resolvers`
  compares the two. Its callers pass the declared base from where it is
  stored: `entity_metadata #>> '{v,scene,base}'` for a Genesis scene,
  `entity #>> '{metadata,scene,base}'` for a world scene.

## Scene bans

`social.scene_bans` (`schemas/40_social.sql`) holds the bans of a scene, one
row for each `(place_id, banned_address)`, under a key stable across redeploys
(`ResolvedRoom::ban_key`): `<world>:<base parcel>` for a world scene (above),
the place of a Genesis scene, the canonical base parcel its admins use. A
Genesis room name carries no deployment; `ResolvedRoom::resolve_ban_key` reads
it for the relay and Cast checks, and refuses a room no deployment stands
behind (an overwritten scene) instead of reading its entity id. The scene-state
socket and its sweep read the key from the deployment they host.

Workload: one primary-key read for each scene join (`/get-scene-adapter`, a
relay admission, a Cast watcher, a scene-state socket and its sweep) and
moderation request; each Genesis relay admission and Cast watcher check adds
one resolver lookup of the deployment (`content.deployments` by entity id);
one read of a
place's rows for a room metadata refresh and a ban listing
(`scene_bans_place_time_idx`). Writes are rare: `POST` or `DELETE
/scene-bans`, and the place removal cleanup.

Invariants:

- A redeploy writes no row. A Genesis redeploy on the same base parcel keeps
  every ban; a scene on another base parcel has none.
- The cleanup of a removed place deletes the bans made before the removal
  (`banned_at`), in the transaction that revokes its admin grants: the rows on
  the place id, the `<world>:<base>` rows of the world (any base, so replaced
  scenes too), or the rows on the base parcel of a Genesis place when no
  enabled `explore.place` row has that base.
- The release script `schemas/releases/genesis-scene-ban-keys.sql` moves the
  bans of Genesis deployments that are not deleted (`deleter_deployment IS
  NULL`, as the resolver reads them, unpublished ones too) from the entity id
  to the base parcel (`social.scene_base_parcel`), keeping the newest ban of a
  wallet when two entities name one place, also against a row the place has.
  The cutover import keeps the newest ban of aliases too. Stop all old
  API replicas before `catalyst schema migrate`, then, with the
  administrative database role, re-run
  `schemas/releases/scene-admin-place-keys.sql` (idempotent: it moves only
  the entity rows left and keeps the newest change of a wallet) and apply
  `genesis-scene-ban-keys.sql`, then start the updated binaries. The re-run
  is needed on a database that ran the admin release before it counted
  unpublished deployments: `ensure_scene_admin_place_release` now refuses
  their entity-keyed grants. `catalyst serve` refuses startup while a ban of
  an active Genesis deployment has the entity key
  (`ensure_genesis_scene_ban_release`). An unpublished deployment's grants
  and bans move to its base parcel, so they apply to whichever scene later
  stands on that parcel. A binary-only rollback is
  not supported after the move; restore the matching backup. A row of an
  entity that is not deployed, or has no parcel that parses, keeps its key,
  and no request reads it. A local preview scene, which has no deployment,
  keeps its entity id. A ban on a scene's least parcel while the scene's
  place is another declared base stays where it is, as an admin grant does
  ("Scene admin grants"), and no longer refuses its wallet; the same operator
  check lists it, and the upgrade re-keys it before players are admitted
  (`infra/containers/README.md`, "Scene admin upgrade").
- A kick intent queued before the release names the entity id; the removal
  job reads its ban again on the scene's place (`legacy_ban_key`).

## Scene stream keys

`social.scene_stream_access` (`schemas/40_social.sql`) holds at most one active
stream key for each `place_id` (`scene_stream_access_active_place_key`). The
place of a Genesis scene's key is the catalogue place based on its base parcel;
the place of a world scene's key is the scene's place
(`ActiveDeployment::place_key`, its `explore.world_scene_place` identity), so
each scene of a world has its own key, streaming into its own room
([ADR](docs/adr/2026-11-21-world-stream-keys-per-scene.md)).

Workload: each scene-signed stream verb and stream link first rechecks the
keys of its place and, for a world scene, of its room (one read each and one
authority decision per key found, usually one or none), then reads the active
key it acts on; a watcher token reads that key alone. Hundreds a day: a
Genesis key by place (the partial unique index), a world scene's key by room
(`scene_stream_access_active_room_idx`), over hundreds of active rows. A
watcher of a world without a parcel, and a stream link of a world scene, count
the world's `worlds.world_scenes` rows up to two; a watcher of a multi-scene
world without a parcel reads the world's active scene-room keys up to two.
A world scene's mint, reset and presenter routes read whether its world has a
listed catalogue place (`explore.place`, `explore.place_world_local`, by
`world_name`, not disabled other than by an opt-out, the reason read from
`explore.place_moderation` by primary key); a revocation whose recheck ended a replaced key reads that
key's room binding (`explore.world_scene_place`, `explore.world_scene_binding`,
by primary key). A held row that cannot move is revoked after one LiveKit
`ListIngress`.

Invariants:

- A world scene's key is the active key streaming into its room. A row under
  the scene's place that streams into another room is not this scene's: an
  imported world-scene identity is a place UUID and can equal the catalogue id
  a world's single key was stored under, so the room, not the place, decides.
  Before a world scene stores a key under its place, such a row moves to the
  place of the scene of its room, which keeps its stream; when it cannot (its
  room has no scene, or that scene has a key of its own) it is revoked while
  idle and still under that place, before any ingress is minted; a live one
  answers 409. The move bumps the row's `streaming_state_version`, so a
  revocation read before a concurrent move is not stored.
- A key minted under the world's catalogue place before keys were per scene
  stays the key of the scene whose room it streams into: that scene reads,
  resets and revokes it, and its replacement is stored under the scene's place.
  It ends at its first replacement, a revocation or its expiry: four days, or
  later while a broadcast that was live at the deadline goes on (at most the
  four-hour streaming cap). Publisher identities keep the place they were
  minted under; a revocation evicts by room and credential, whatever the place.
- A removed world ends every active key whose room is one of its scene rooms
  (`starts_with(room_id, <world room prefix>)`), the catalogue place's
  included. Any removal event of a world place does, also one for a single
  scene's Places row, as the grant cleanup revokes the grants of every scene
  of the world. A removed Genesis place ends its own key and no key streaming
  into a world scene room.
- The notice of a world scene key's cleanup names the world (its catalogue
  place, else the world alone) with the footprint of the scene of the key's
  room, so the holders of its parcels are told; its owner is the world's.
- Of several active keys streaming into one room, every reader by room
  takes the one under its own scene's place (`place_id = admin_place_key`),
  then the newest (`streaming::ROOM_KEY_ORDER`): the verbs and the presenter
  routes act on the same key.
- A held row is moved only to a current scene (one with an identity), and
  revoked only while LiveKit reports its ingress idle.
- Rollout: stop the old API replicas before the new binaries serve. An old
  replica stores a world key under the world's catalogue place, with its
  scene in `admin_place_key`; the `BEFORE INSERT` trigger
  `scene_stream_access_scene_place` refuses a `w1:` room row whose
  `admin_place_key` is set and differs from `place_id`, so its mint fails
  instead of adding a second key to the room. Imported rows (`admin_place_key`
  NULL), updates and stored rows are untouched. Drop the trigger before
  rolling back to an older binary.

## Stream key notifications

A Cast stream key (`social.scene_stream_access`) is stale when the scene of its
room was deployed again or removed, or when its minter lost the place. The
gatekeeper checks a key on each use and in its key job
(`comms-streaming-key-recheck`: 20 keys each 600 s in place order, so every
key in `ceil(keys / 20) * 10` minutes; its cursor moves with each key and
lives in the process, and the expiry of keys is a job of its own). Its listener
(`gatekeeper::streaming::cast::listen`, the leased job
`comms-streaming-key-listener`, so one `serve` process) runs that check at once
for the place a write names, on two channels:

- `new_deployment`, payload `<entity type>:<entity id>`: each
  `content.deployments` insert and `content.unpublish_scene_*`. The listener
  keeps the scenes and checks the key of the Genesis scene that the entity is
  or replaced (`content.deployments` by `entity_id` and `deleter_deployment`).
- `world_authority`, payload `world:<name>`: `worlds.notify_world_authority`
  on a local scene inserted in or deleted from `worlds.world_scenes`, an owner
  change in `worlds.worlds`, a `worlds.world_permissions` delete and a
  `worlds.world_permission_parcels` insert (a grant narrowed to parcels no
  longer holds the world's key). The listener checks the keys of the world's
  scene rooms.

Workload: a synced node sends hundreds to a few thousand scene messages a day,
thousands in a burst during a first sync; world writes are tens a day and the
world mirror sends none. One message is one read of the active keys of its
place (`split_part(room_id, ':', 3)` or `starts_with(room_id, ...)` over the
active rows, hundreds at most) and one authority decision for each key found,
usually none. The check of every key at a subscription is one authority
decision for each active key: a LAND or NAME read, or a world permission
read, each bounded by its own timeout.

Invariants:

- `LISTEN` belongs to a session: the listener holds one connection of the
  serve pool.
- A payload names a public place: no address, no secret. Every role may
  `LISTEN` and `NOTIFY`; a payload only selects keys for the check, which reads
  the authority again. The world payload is cut at 1000 characters, so
  `pg_notify` (8000 bytes) never fails the write.
- PostgreSQL keeps no message for a session that is away: each subscription
  starts with a check of every active key, and the key job stays.
- The payloads that wait are a set: the messages for one place that arrive
  during a check are one more check, not one for each message.
- Two listeners during a lease change are safe: a revocation is one
  conditional update of the key row.
- An existing database gains the function and its triggers from the object
  pass of `catalyst schema migrate`; no step exists.

Not announced: a LAND owner or operator change, a NAME transfer until the
owner reconcile writes `worlds.worlds.owner`, a ban. The key job and the check
on each use cover them. A scene admin removal checks the key in its own
request.

Streaming notification fanout uses the same owner and permission authorities
as access decisions: LAND owner/operator/update operator/update managers and
approved-for-all wallets, or World owner and footprint-scoped grantees, plus
scene admins. Wallets are normalized and deduplicated. Invalid historical
admin strings are excluded from delivery without preventing safety cleanup.
Each credential stores its bound `admin_place_key` at issuance/reset/Cast;
the cleanup context preserves it for later resolution. Notification preparation
is bounded to five seconds, retains partial evidence on failure and never takes
publication row locks. Original place context is captured before binding recovery
and checkpointed before further fallible deferred lookups; saved snapshots
survive catalogue deletion and are never refreshed from a replacement lifetime.
Binding SQL errors and timeouts leave recipient resolution pending, including
retries after credential deactivation.

Explore owns `world_scene_binding`: one recovery fact for each distinct
`(world_name, entity_id, identity)` displaced by identity replacement,
retirement or deletion. Its row trigger inserts the old exact binding in the
identity transaction, with no Social writes or row locks. A rollback retains
neither transition nor evidence. The primary key in that order supports the
exact world/entity probe and makes replay idempotent. There is no FK because
these facts survive deletion of the source identity. There is no expiry while
legacy credentials or pending notices may need them; storage grows with
distinct displaced bindings. Comms reads these columns together with the
current binding, considering at most two historical identities and the one
active identity. A saved credential/context key takes precedence. Without one,
a successful lookup returning zero or multiple candidates records
`admin_binding_pending=false, admin_binding_unresolvable=true` in
`source_context`. Such canonical World notices complete for independently
established owner and permission holders plus preserved recipients, skipping
local-admin enrichment so parcel overlap cannot select another incarnation.
SQL errors and timeouts remain retryable instead. Before the place-context
or binding lookup, preparation freezes the `social.scene_admins` [admin,
granter] pairs of each candidate key (the saved key, or else those same
historical and active identities) in `source_context.admin_grant_candidates`;
the retry keeps the pairs of the key it settles on, so a re-grant in between
cannot replace the original granter. The
schema object pass installs this new table, function and trigger; imports
retain facts through the same trigger. Production insert and retention costs
are unmeasured.

`scene_stream_cleanup`
keeps the complete `recipients`, `recipients_resolved` bit and guarded
`recipient_cursor`. Each Social acceptance advances one cursor under the
current claim token, so a bounded pass resumes after its accepted prefix.
`notification_started` commits before the first acceptance; later key regeneration
cannot suppress an unfinished expiry suffix, including an uncheckpointed acceptance.
Social deduplicates by recipient, kind and event identity. Reset-to-revoke
conversion retains the complete recipient set and starts the new kind at zero.

Existing credentials without a saved admin key can recover a unique exact
current or retained binding, including after credential deactivation and scene
retirement. Scenes retired before binding capture existed and overwritten
pre-cutover grant history may have no exact recovery evidence. Existing scalar
cleanup recipients are preserved while remaining authority classes resolve;
an existing scalar value alone never marks the snapshot complete.

Credential issuance has no per-place limit, as upstream; historical imports
preserve creation times. Whether to bound it is open next work in the
[streaming notification ADR](docs/adr/2026-11-21-streaming-notification-recipients.md).

## Stream state reconciliation

The streaming job (`comms-streaming-ttl`, each 60 s) makes the `streaming`
flag of `social.scene_stream_access` agree with LiveKit when a webhook was
lost. One pass reads at most 20 streaming credentials (flag up, no stream: a
lost `ingress_ended`), then at most 20 idle ones (flag down, the ingress
publishes: a lost `ingress_started`), each kind the ones checked longest ago
on its own partial index: `scene_stream_access_reconcile_idx` (`WHERE active
AND streaming`) and `scene_stream_access_reconcile_idle_idx` (`WHERE active
AND NOT streaming`). Each row read is one LiveKit `ListIngress` call and one
update of `streaming_checked_at`.

Workload: active credentials are one for each place with a key, hundreds at
most; streams live at once are tens. A streaming credential is read again
within `ceil(streaming / 20)` minutes and an idle one within
`ceil(idle / 20)` minutes: the idle keys do not delay a live stream.

An existing database gains the idle index from the object pass of `catalyst
schema migrate` (a new name, built in the migrate transaction on a table of
hundreds of rows); no step exists and no index is replaced.

## Player report intake workload

The player report intake (the node area `/report-user`,
`services/catalyst/comms/src/gatekeeper/reports.rs`) serves a human writing a
handful of rows into `social.player_reports` and
`social.player_report_evidence` (`schemas/40_social.sql`). The Sites report
form (`/report`, `apps/sites/src/data/lib/catalyst/landings/report.ts`) posts
to `/comms/reports/players`, which the node does not serve, so the intake has
no caller in this tree; pointing the form at
`/report-user/api/reports/players` is the next work. The order is
slots first, report last, so the tables share no FK: a presign inserts one
to five evidence slots under a fresh report id (`report_id`, `evidence_key`
`<position>-<sanitized filename>`, `reporter_address`, the declared type and
size, the client address bucket `source_key`) in one transaction; each
evidence `PUT` (at most 5 MiB, the node's cap and half the form's 10 MB;
exactly the declared size, the declared type in the header and a first bytes
that match that type) claims the slot under its row lock (`content_hash` and
`uploaded_at`, set only while they are empty) and then stores the bytes in
the shared hash-addressed blob store; the filing locks the slots
(`FOR UPDATE`), checks every planned key is claimed with its bytes in the
store and inserts the `player_reports` row (`status`
`open`, `evidence_keys` the slot keys in order). Volume is a few reports a
day; every statement of these three steps is a primary-key read or write,
beside the limit counts below. The moderator read
(`GET /api/reports`, the moderator token or a listed moderator wallet) lists
filed reports newest first (`player_reports_created_idx`), optionally for one
reported wallet (`player_reports_reported_idx`), and loads their evidence
rows with one `= ANY` by report id, in `evidence_keys` order (50 reports a
page, at most 200, so one answer holds at most 1,000 evidence rows); the
evidence bytes come from the store by `content_hash`, as a download the
browser neither sniffs nor runs (`nosniff`, `Content-Disposition:
attachment`, `Content-Security-Policy: sandbox`).

Limits, each checked under a per-reporter transaction advisory lock so two
concurrent requests cannot both pass: a wallet files at most 5 reports per
rolling hour (`REPORTS_PER_HOUR`, counted on `player_reports_reporter_idx`;
the presign refuses early and the filing refuses again, both `429`; a person
writes each report by hand, and five an hour is above the node's volume of a
day), and holds at most 25 unfiled slots minted in the last 24 hours, the
evidence of one hour's reports
(`PENDING_SLOTS_PER_REPORTER`, counted on
`player_report_evidence_reporter_idx` against `NOT EXISTS` of the report
row; `429` at presign). Slots older than that window no longer count, so an
abandoned form never locks a wallet out for good.

A fresh key is a new wallet, so the wallet limits alone bound nothing. Each
client address (the one `serve` pins to the socket peer or takes from a
trusted proxy, IPv6 by its /64) therefore has two budgets that no key
escapes, both buckets that refill over a day, held in the memory of each
`serve` process: 100 presigned slots (`SOURCE_SLOTS_PER_DAY`, four wallets'
pending quotas, for the people one address stands for) and 500 MiB sent to
the store (`SOURCE_KIB_PER_DAY`, those slots filled once at the file cap;
every `PUT` of the declared size and type spends its size, also one the slot
then refuses). Both refuse with `429`.

A sender with many addresses has as many budgets, and a process that
restarts forgets them, so one bound holds for the whole node, and each
client address bucket has a share of it. Workload: one more read per
presign, after the wallet quotas and the address budget pass: the sum of
`greatest(declared_size, 262144)` over the slots that count, for the node
and, as a `FILTER` of the same statement, for the presign's `source_key`. A
slot counts while it holds a file and is younger than 24 hours (a range read
on the partial index `player_report_evidence_uploaded_idx`), or while it
holds none and is younger than 15 minutes (`RESERVATION_MINUTES`; a range
read on `player_report_evidence_created_idx`); a normal day has a few rows
in both. The first `PUT` of a slot adds one lock and no read. The 15 minutes
are a policy, not a measurement: the form sends its files right after the
presign, and five files at the cap in that time need less than 230 kbit/s.
Invariants: the presign takes one transaction advisory lock
(`player-report-evidence:node`) before the sum and holds it over its
inserts, and the first `PUT` of a slot takes the same lock before its claim,
so every `serve` process counts the same rows and no two requests pass
together; the claim is refused (`410`) when the slot is older than the 15
minutes on the statement clock read under that lock, so a slot that left the
sum never takes a file; a slot takes one body of its declared size (below),
so the stored bytes of the slots created in any 24 hours never pass the sum;
the sum plus the new slots never passes `REPORT_EVIDENCE_DAILY_MIB` (default
2,048 MiB; the value is the operator's policy for the store space reports
may take in a day, not a measurement; 0 accepts no evidence); the slots of
one address bucket plus the new ones never pass that address's share, a
sixteenth of the bound (`NODE_SHARE_DIVISOR`; 128 MiB at the default, the
evidence of one wallet's hour) and at least one report's files at the cap
(`NODE_SHARE_FLOOR_BYTES`, 25 MiB), so fewer than sixteen addresses never
close the intake to another one (a policy, not a measurement); the sweep
deletes no slot younger than the 24 hours; each slot counts for at least
256 KiB (`NODE_SLOT_FLOOR_BYTES`), so each of the two ranges holds at most
the bound divided by that floor (8,192 rows at the default), which is the
most the sum reads. The node lock is the last lock a request takes: after
the reporter lock in a presign, after the slot's row lock in a `PUT`. At the
bound or at the share a presign answers `429` and keeps the address's
spend. Filed reports keep their evidence without a time limit: no code moves
a report out of `open`, so no retention rule exists yet.

`source_key` is the bucket key of the presign's client address and nothing
more of the request: an IPv4 address as it is, an IPv6 address cut to its
/64, empty when `serve` has no address. Only the sum above reads it; no
route returns it. Each presign clears it on up to 100 slots that hold a file
and are older than the 24 hours (oldest first on the partial index
`player_report_evidence_source_idx`, `FOR UPDATE SKIP LOCKED`), and a slot
without a file loses it when the sweep below deletes the slot. So a key
stays for 24 hours and then until the next presigns; a node that receives no
presign clears nothing. The step `0046_player_report_evidence_source_key`
adds the column; the slots from before it keep `NULL` and count for the node
and for no address.

Each presign then deletes up to 100 unfiled slots older than the 24 hours
(`SWEEP_BATCH`, twenty presigns' worth), oldest first on
`player_report_evidence_created_idx`, so the store's collector frees their
bytes. It locks the slots it takes with `FOR UPDATE SKIP LOCKED` and deletes
them in a second statement that checks `NOT EXISTS` of the report row again:
a filing holds its slots' locks until its report row is committed, so the
sweep never deletes the evidence of a filed report. The scan passes the
slots of filed reports older than the window (one primary-key probe each),
which at a few reports a day stays a small range. A report not filed within
the 24 hours is gone: its `PUT` and its filing answer `404`.

Invariants: a slot's `reporter_address` is the wallet that signed the
presign and never changes; an evidence `PUT` and the filing are refused for
any other signer (`403`), and once the report row exists (`409`); a filing
names exactly the presigned keys, all uploaded, so a filed report has
complete evidence; the reason is one of the form's five values and the texts
keep the form's 500-character limits; a slot takes one body: the claim sets
`content_hash` only while it is empty and only the claiming `PUT` writes to
the store, the same bytes again answer `200` (`409` until the first write
ends), other bytes answer `409`, and a store that fails clears the claim, so
the slot takes a `PUT` again in its 15 minutes. Slots whose client never files are deleted by the sweep
above; the queue reads filed reports only. The
bytes are content-addressed and rooted by `content_hash`
(`services/catalyst/storage/src/gc.rs`), so two reports attaching the same
file share one blob and a report's deletion never removes bytes another one
references.

## Sites pairing sessions workload

`explore_api.pair_sessions` (`schemas/11_explore_api.sql`) holds the phone
pairing sessions of Sites (`/auth/pair/:session` and `/internal/pair`,
`apps/sites/src/data/lib/auth/pair-store.server.ts`), so a create, the phone's
complete and the desktop's polls may land on different Sites replicas behind
the gateway's round-robin `sites` upstream. Sites is the only reader and
writer, through its catalyst pool (`CATALYST_DATABASE_URL`, shared with the
places reads); the node never touches the table. It sits in `explore_api`
beside the other explorer sign-in surface tables and shares their lifecycle:
rows are disposable, and a restore may lose them.

Inputs: a desktop creates a session (rate-limited per client IP to 30 in five
minutes, in each Sites process), polls it every few seconds, and the phone
completes it once with a signature. One row per session: id (16 random bytes,
base64url), the sha256 of the 32-byte poll token (the token itself is handed
to the desktop once and never stored), the ephemeral address, its expiration
and the message to sign, the creation and expiry instants and, once
completed, the signer and signature. A few hundred bytes a row; at most 500
live rows by design.

Transitions, each one statement against the database clock (`now()`):
create takes `pg_advisory_xact_lock(hashtextextended('sites-pair-sessions',
0))`, deletes the expired rows, and inserts only while fewer than 500 rows
remain, so concurrent creates on several replicas never exceed the cap and
never evict a live session; complete is `UPDATE ... WHERE completed_signer IS
NULL AND expires_at > now()`, so exactly one phone wins (`already` for the
others, `expired` or `missing` after a lookup); poll reads the row by id,
compares the token's sha256 to the stored digest in constant time, and
releases a completed session by deleting it with `RETURNING`, so one poll
receives the signature; cancel deletes by id and token digest. Expired rows
are deleted by the next create, and by a poll or complete that finds one;
no job sweeps them, so between creates they stay bounded by the creates of
the last five minutes. No secondary index: every read is by primary key and
the sweep and count scan at most a few hundred rows.

Without a catalyst database (`CATALYST_DATABASE_URL` and
`PLACES_PG_COMPONENT_PSQL_CONNECTION_STRING` unset), Sites keeps the sessions
in its own memory with the same rules, like the places reads fall back to the
API; that mode serves one process only, so a deployment with several Sites
replicas sets the database URL. The bootstrap defines the table; an existing
database gains it from the object pass, so no step exists.

Before exposing writers, a Social-writing serve process scans active native
credentials with missing admin keys in bounded UUID batches, decodes their
canonical rooms with the existing codec, and saves exact live world identity
bindings under world/identity locks. Read-only Social replicas skip this write.
The upgrade requires stopping old writers and finishing offline imports before
the coordinated startup, so identities cannot retire before this backfill.
Noncanonical imported rooms and scenes already retired before the upgrade
remain outside exact recovery; pending authority stays retryable where available.

## Shop reconciliation and availability sweeps

The broker purchase, NAME transfer and escrow-action receipt workers read up
to 100 unsettled rows per chain and pass, the escrow release worker 50 due
grants, the credit-revocation worker 20 unfinished revocations per chain,
and the availability watcher 500 distinct pending item keys. A pending
receipt, malformed grant or unknown mirrored item remains an obligation;
it must not permanently occupy a bounded query's first page.

`shop.worker_progress` stores each worker's position and a frozen upper key
for its current sweep, scoped by chain where applicable. The workers use
indexed bigint ids, credit hashes, or `(contract_address, item_id)` pairs
without converting indexed columns to another type. Selection and cursor
advance share a short transaction under the progress row's lock. Chain
calls, release attempts and item observations run after that transaction
commits. A crash after selection defers that page until the next sweep;
it does not delete work, spend an attempt or mark a receipt successful.
The item watcher also freezes the largest request identity and excludes
later request ids even if their item keys sort inside the lexical range.
Revocations freeze a database clock cutoff and exclude rows created after
it; their sole enqueue path uses the database `created_at` default and does
not backdate new work. A transaction already open at that cutoff can still
commit an earlier-dated row, which is revisited on wrap if its key was
passed. Newly started transactions cannot extend the cohort. New arrivals
outside these bounds wait for the next sweep, so continuous arrivals
cannot indefinitely delay retry of a blocked prefix. Rows that
commit or become due behind the position are also revisited after wrap.
The existing status, attempt limits, settlement grace and observation
timestamps keep their meanings. Existing idempotent writes and job leases
still govern effects; the sweep cursor is not an external-call lease.

The table has no foreign keys to individual obligations: completed or
deleted source rows may disappear between pages. An empty remainder ends
the cohort and starts another. Its size is one row per worker and chain,
not one row per obligation. The schema object pass installs the new table
from the bootstrap definition. The credit worker uses `credits.revocations_chain_hash_idx`
on `(chain_id, credit_hash)` for both its upper bound and each hash-ordered
page. If new arrivals greatly outnumber rows eligible for an ongoing sweep,
the page query can instead use `credits.revocations_due_idx` to select just
the rows created on or before the sweep's fixed start time, then sort those
hashes. Both indexes contain only pending and sent rows. The partial index adds write
work when revocations enter or leave those states. See [the measured query
plans](METRICS.md#credit-revocation-sweep).

The object pass creates the index in the migration transaction, which blocks
writes to `credits.revocations` until commit. On a busy database it can be
built before migration using the schema definition:

```sql
CREATE INDEX CONCURRENTLY revocations_chain_hash_idx ON credits.revocations
    (chain_id, credit_hash) WHERE status IN ('pending', 'sent');
```

Verify the definition and `pg_index.indisvalid` before running migrate.
The object pass checks its name. Drop an invalid index before retrying.

Escrow reconciliation applies its missing-grant `NOT EXISTS` predicate
before `LIMIT 500`. Any grant with that reference satisfies the check. Catalogue
and rental pages count their filtered set independently of page offset in
the same SQL statement. The count remains present even
when the page is empty. Both sides therefore use one snapshot and one
filter definition. Production query latencies and cardinalities for these
repairs remain unmeasured.

## Wearable and emote ownership workload

The shop's ownership reader (`services/catalyst/shop/src/market/owned_items.rs`)
answers who holds which collection item from the nft index's fold, `shop.nft`,
which the shop writes itself; it reads no other area's table. Readers:
profiles and the Registry's profile batches (`/lambdas/profiles`, `POST
/profiles`), the wearable and emote lists (`/lambdas/users/{a}/wearables`,
`/emotes`, `/lambdas/collections/*-by-owner`), the explorer backpack,
`/v1/users/{a}/wearables` and `/emotes`, outfits, the `/v1/orders` owner
guard and the wearable and emote pages of `/v1/nfts`. Content deployment
validation does not read it: it checks ownership by RPC at the entity's block
and `shop.usage_grants`.

Readiness, read on the cursor (`shop.chain_cursor`, `indexer = 'nft'`) in the
same `REPEATABLE READ, READ ONLY` transaction as the rows, so a rewind that
commits in between is never half-seen:

- complete: `full_history` and `folded_block` set. A reorg rewind and an
  unpublishable fold clear `folded_block`; only a pass that reaches the head,
  folds and publishes sets it, and a rewind deletes the fold's rows in the
  transaction that clears it. `shop.nft` changes only in a fold's commit
  (one transaction per network) and in a rewind, so a reader never sees half
  a fold, and a complete cursor holds while the next pass has advanced
  `last_block` past `folded_block`. A fold that still owes a late
  collection's backfill is unpublishable: it holds that collection's tokens
  transferred since the walk took it in and not its earlier ones. Its
  transaction clears `folded_block` and `folded_at` with its rows (the
  count of owed backfills is read in it, and the cursor's update is its last
  statement), so those rows are never read as complete, whatever stops the
  pass afterwards; the pass that finishes the backfill folds and publishes
  again. A fold that owes nothing leaves the marker to the pass, which sets
  it, or clears it when the pass is held short of the head or past the
  publish lag, in a later statement after it reads the chain head again:
  until then, and until the next pass folds when that read fails, the new
  fold's rows are read under the previous marker and its `folded_at`. They
  are later transfers of the same complete walk. Profiles need complete
  on the networks their items name; an incomplete one fails the read (the
  404 of a failed lookup, or a Registry batch left out and counted).
- fresh: complete and `folded_at` within 30 minutes. Every other reader needs
  it on the networks it reads (wearables both, emotes Polygon), and the lists
  also need the items mirror's complete pass (`shop.mirror_state`); otherwise
  they answer the `nft-index` envelope. The `/v1/orders` routes instead
  answer 200: until the index is fresh on the networks the read names, their
  v3 listings leg keeps only the Ethereum LAND and Estate registries'
  listings, which the index never holds (it reads their `Update` logs, not
  their transfers) and the guard never filters, and the
  `x-listings-owner-check` header says `partial` (`checked` when the guard
  ran, `unchecked` on a node that does not walk the index). Which contracts
  the index holds is not read from `shop.nft`: a rewind deletes the rows of
  the tokens it reaches, so a sold token's collection could look unindexed.

Queries, all bounded by one owner or one batch:

- an owner's copies: `shop.nft` by `owner` (`nft_owner_idx`), joined to
  `shop.items` by `(network, contract_address, item_id)` on Polygon and to
  `shop.collections` and `shop.items` by URN (the collection's URN and the
  Issue log's `wearable_id`) on Ethereum; memoized 30 s per owner and
  category. A request opens one snapshot: the gate is checked on it every
  time, the rows are read on it, and a memoized answer runs no query after
  the gate;
- the profile batch: up to 128 profiles' `(owner, URN)` pairs unnested, v2
  pairs joined to `shop.nft` on `(network, contract_address, item_id)` and
  `owner`, v1 pairs through `shop.collections.urn` and `wearable_id`, newest
  transfer first (`transferred_block`, `transferred_log` descending);
- the orders guard: `NOT EXISTS` a `shop.nft` row of the listing's exact token
  held by another address or burned, by primary key. A node that does not
  walk the index runs the listings statement without it and without the
  snapshot.

Measured on a seeded database (3.0M Polygon tokens over 100,000 items with a
skewed popularity, 300,000 owners of whom 20 hold about 30,000 tokens each,
200,000 Ethereum v1 tokens and 300,000 names; PostgreSQL 18, warm cache):

- the profile batch: each `(owner, URN)` pair is a `BitmapAnd` of
  `nft_owner_idx` and `nft_item_idx`; 128 owners by 10 URNs took 220 ms, one
  profile of 10 items 17 ms, and a 30,000-token owner wearing the ten
  most-held items 21 ms. An index on `(owner, network, contract_address,
  item_id)` cut those to 22, 0.2 and 0.9 ms, but at 442 MB against
  `nft_owner_idx`'s 55 MB, and it made the planner read a 30,000-token
  owner's inventory in 1.35 s instead of 89 ms, so it was not added;
- an inventory read: 89 to 99 ms for a 30,000-token owner, 0.2 ms for a
  typical one;
- `/v1/nfts`: an owner's `category=wearable` page 95 ms and its capped total
  131 ms for the 30,000-token owner, under 1 ms for a typical owner (with the
  names leg when no category is named).

On a configured NFT walker, a LAND or estate `/v1/nfts` page reads the LAND
publication and the required NFT publications in one repeatable-read snapshot.
Both owner searches and ownerless catalogue pages read the collections, NAME
and LAND sources that their filters can include. This includes unfiltered
browsing, search, sale filters and sorting. A missing required publication returns its source's 503
for the whole page. The rental-status filter, page, count, estate members,
rental listings and open order enrichment all use the same snapshot. An empty
rental filter result is returned only after the publication checks.

An owner filter that includes LAND also reads the Rentals publication for its
Ethereum chain. On Sepolia the raw history uses the `rentals_sepolia` cursor
and `sepolia` rental events. Signed listing enrichment also requires the
`rental_listing_generation` row in `shop.worker_progress` to match that
cursor’s owner generation. A newly attached catalogue writer refolds the
existing archival events before publishing signed listing statuses.
It finds the wallet's candidate rents with `rental_event_lessor_idx` and the
latest published rent or claim per asset with `rental_event_asset_idx`.
An unclaimed rent counts past expiry while the venue owns the token and the
rent covers its latest transfer into custody. Each event query stops at its
source's published height. Owner pages can use the last fresh Rentals fold
while the walker processes a newer window. A published claim ends the rental
match immediately. Direct ownership becomes visible when LAND publishes the
transfer back. Before routing owner inventory to a walker, check
`fullHistory` and `ready` for `ethereum` or `sepolia` at
`GET /signatures-api/v1/rentals-index` on that node. If Rentals has yet to publish
through a transfer into the venue, the owner page returns the `rentals-index`
503. Both page and count include these assets, and the NFT still reports its
on-chain owner.

The `/v1/nfts` page without an owner reads each source's first `first +
skip` rows under the filters and the sort, then sorts and pages their union,
which always holds the window of the whole set. Each collections and names
source walks `nft_mint_order_idx` (`(network, minted_at DESC) WHERE owner IS
NOT NULL`; an incremental sort orders the ties by contract and token) with
nested-loop lookups of the item, the collection and the legacy order. A read
that keeps only listed rows (`isOnSale=true`, a price bound) draws its
candidates from the tokens with an open legacy order or v3 listing
(`on_sale`, materialized once) and still applies the order filter, so it
answers the same rows; the LAND source names those tokens on the registry's
own rows, so its per-token lookups run for the listed parcels only. The
capped total reads the order legs only when a filter needs them: they are
left joins of at most one row, so a sort on them changes no count.

A sort by price or listing time (`cheapest`, `recently_listed`) puts the
rows with no order last, by contract and token. Without an owner and
without a filter that keeps only listed rows, the page reads each source as
two parts: its rows with an order, drawn from `on_sale` under the sort, and
its rows without one, read in the source's key order (`shop.nft`'s and
`land_authz.token_right`'s primary keys), each part giving its first `first
+ skip` rows. The two parts hold every candidate of the source, so their
union holds the window of the whole set, and no read prices every
candidate. With `isOnSale=false` only the second part is read. An owner's
page keeps one part unless the owner holds more than 1,000 tokens
(`LARGE_OWNER_TOKENS`, counted in the read's snapshot): a typical owner's
price-sorted page takes 10 ms that way and 240 ms split (the cost of
reading every listed token first), a 32,000-token owner's 7 to 8 s and
0.6 s.

Each candidate row's v3 listing is a lateral lookup in `unified_trades`,
which these statements declare `NOT MATERIALIZED`: the lookup reads the
listings of that token through `trade_assets_erc721_token_idx` on
`shop.trade_assets_erc721 (token_id)` and the trades its owner signed
(`idx_trades_signer_created`), instead of scanning every open listing for
every row. The index is 432 kB on 4,900 listing assets (the table 528 kB);
each ERC-721 asset that the trades mirror or a local signed trade inserts
adds one entry, and the migrate object pass builds it. Planning such a
statement takes about 9 ms.

Measured on a second seeded database (2,999,100 Polygon tokens over 100,000
items of 5,000 collections, item popularity cubic-skewed so one collection
holds 175,000 tokens and the smallest about 200; 200,000 Ethereum v1 tokens
and 300,000 names; 20 owners of about 32,000 tokens among 300,000; 64 tokens
share each mint time; 90,000 parcels and 2,000 estates; 4,450 open legacy
orders and 4,600 served v3 token listings, some signed by an address that
no longer holds the token; PostgreSQL 18, warm cache, `first=24`, each
statement run through the route's query builder with bound parameters).
Its rows were inserted in mint order, so the heap follows
`nft_mint_order_idx` (43 MB here, 105 MB on the earlier seed) and the deep
windows below are a lower bound for a table folded in another order.

| Page without an owner | Page | Capped total | Where the time goes |
|---|---|---|---|
| newest, `category=wearable` | 30 ms | 92 ms | the Ethereum source passes the 238,000 names minted after the newest v1 token (22 ms) |
| newest, no category | 291 ms | 88 ms | the LAND source reads every parcel's mint before sorting |
| newest, `category=wearable&isOnSale=true` | 767 ms | 785 ms | 8,200 listed tokens, each with its token, item, order and listing lookups |
| newest, `isOnSale=true`, no category | 838 ms | 822 ms | the same, with names and LAND |
| cheapest or recently listed, `category=wearable&isOnSale=true` | 745 to 770 ms | 730 ms | the same |
| newest, `category=wearable&isOnSale=false` | 1.0 s | 0.5 s | the listing lookup of each row walked |
| newest, `category=wearable`, `skip=1,000` | 269 ms | 78 ms | 1,024 rows of each source |
| the same, `skip=10,000` | 916 ms | 76 ms | 10,024 rows of each source |
| the same, `skip=99,976` (the 100,000 window) | 2.6 s | 71 ms | 100,000 rows of the Polygon source with their lookups, the Ethereum source sorted on disk |
| newest, no category, `skip=10,000` | 1.6 s | 91 ms | four sources of 10,024 rows |
| the same, `skip=99,976` | 3.9 s | 95 ms | 392,000 rows sorted on disk (130 MB) |
| newest, one creator of 3 items | 75 ms | 73 ms | drives from `items_creator_idx`; the Ethereum source reads its 200,000 tokens |
| newest, `itemRarity=unique` (40 tokens) | 107 ms | 112 ms | drives from the items of that rarity through `nft_item_idx` |
| newest, `itemRarity=mythic` (85,000 tokens) | 204 ms | 222 ms | walks `nft_mint_order_idx` |
| name, `category=wearable` | 380 ms | 78 ms | the items sorted by name, then `nft_item_idx` |
| name, no category | 0.7 to 1.2 s | 96 ms | the same, and 300,000 names sorted |
| recently sold, `category=wearable` | 1.6 to 3.3 s | 80 ms | every held token read and sorted: no index orders `sold_at` |
| recently sold, no category | 2.1 to 2.9 s | 94 ms | the same |
| cheapest or recently listed, `category=wearable` | 736 to 740 ms | 65 ms | the listed part (8,200 listed tokens with their lookups); the unlisted part reads 24 rows |
| the same, no category | 791 ms | 82 ms | the same, with names and LAND |
| the same, `category=wearable`, `skip=1,000` | 749 to 755 ms | 65 ms | 1,024 unlisted rows of each source added |
| the same, no category, `skip=1,000` | 918 to 985 ms | 83 ms | the same over four sources |
| the same, `category=wearable`, `skip=99,976` | 2.4 to 2.5 s | 67 to 75 ms | 100,000 unlisted rows of each source with their lookups |
| the same, no category, `skip=99,976` | 8.1 s | 83 ms | the LAND source's 91,600 unlisted rows with their per-token lookups (4.4 s), 400,000 rows sorted on disk |
| cheapest or recently listed, `category=wearable&isOnSale=false` | 19 to 21 ms | 458 to 509 ms | 24 rows in key order |

A page sorted by price or listing time that read every candidate before
the sort ran past the pool's statement timeout (`PoolSettings`, 60 s by
default) on this seed. With `unified_trades` materialized, no token index
and the LAND source unrestricted, the same seed took 2.9 s for the
wearables on sale (page and total), 30 s for the on-sale page naming no
category, 59 s for `isOnSale=false`, and 30 s for the capped total of a
sort by price. Every statement of these reads (`/v1/nfts`, the nft leg of
`/v1/prices`, the estate sizes) runs under a transaction-local
`statement_timeout` of 10 s (`READ_STATEMENT_TIMEOUT`), above the slowest
ones measured here (8.1 s), and a cancelled one answers 503 naming the
bound. The bound is per statement: a page runs at most four heavy ones
(the owner's count, the page, the capped total, the estates' parcels), so
it holds its connection up to 40 s rather than the pool's 60 s for each.
Planning a split page takes 15 to 23 ms.

With an owner: a 32,000-token owner's `category=wearable` page 78 ms and
total 167 ms, its `isOnSale=true` page 277 ms, and its page sorted by price
7 to 8 s read in one part, the listing lookup of each of its tokens (0.6 s
in two parts, as such an owner's page is now read), and its capped total
with no category 7 to 8 s. A statement
repeated on one connection is planned generically from its sixth run: that
plan took 1.0 s for the 78 ms page and 1.3 to 1.6 s for the price-sorted one.

The earlier seed (3.0M Polygon tokens, 3,500 legacy orders, no v3 listing)
gave, before and after the per-source windows: newest wearables 1,605 ms to
15 ms, newest with no category 5,138 ms to 282 ms. `sold_at` of the LAND
source is a scalar subquery the plan evaluates for the page's rows only.
`the_per_source_window_pages_like_the_whole_set` compares the page with the
whole set sorted and paged over every sort, filters and pages inside and
past a source's rows, with legacy orders, v3 listings and a listed parcel,
and checks the totals. The index on `(network, minted_at DESC,
contract_address, token_id)` answered within 6 ms of `nft_mint_order_idx` at
396 MB against 105 MB. Unmeasured: production data, a cold cache, and
concurrent reads.

`catalyst schema migrate`'s object pass builds `nft_mint_order_idx` on
`shop.nft` (no step under `schemas/migrations/`) in the migrate transaction,
not concurrently, so writes to that table wait for the build: about 2 s on
the 3.5M seeded rows, unmeasured on production. Writers are stopped for a
release anyway. To build it outside that window, run beforehand, with the
same definition as `schemas/30_shop.sql`:

```sql
CREATE INDEX CONCURRENTLY nft_mint_order_idx
    ON shop.nft (network, minted_at DESC) WHERE owner IS NOT NULL;
```

The object pass creates only objects the database lacks, so it then skips the
index. A failed concurrent build leaves an invalid index of that name: drop it
before retrying. Each nft fold maintains the index beside `nft_owner_idx` and
`nft_item_idx`.

The DCL NAME owner read (`services/catalyst/shop/src/market/names.rs`) looks
a label up lowercased, through `nft_ens_label_idx`, which only the
DCLRegistrar's rows enter (`ens_subdomain` set). The object pass builds it the
same way; to build it outside the release window:

```sql
CREATE INDEX CONCURRENTLY nft_ens_label_idx
    ON shop.nft (lower(ens_subdomain)) WHERE ens_subdomain IS NOT NULL;
```

Its size and the lookup time on production data are unmeasured.
