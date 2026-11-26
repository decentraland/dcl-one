# builder-server -> builder

Pinned at `98897af692f26063a088aebb2ea5bb3f683168a2` (migrations through
`1790640000001_create-collection-events`).

| Step | Upstream | Target |
| --- | --- | --- |
| `collection` | `collections` | `builder.collections` |
| `item` | `items` + newest `item_curations` (content hash) + parent `collections` | `builder.items` |
| `item_content` | `items.contents` (file -> hash) | `builder.uploaded_contents`, `builder.item_contents` |
| `virtual_third_party` | `virtual_third_parties` | `builder.virtual_third_parties` |
| `collection_curation` | `collection_curations` | `builder.collection_curations` |
| `item_curation` | `item_curations` | `builder.item_curations` |
| `collection_event` | `collection_events` | `builder.collection_events` |
| `slot_usage_cheque` | `slot_usage_cheques` | `builder.slot_usage_cheques` |

## Derived columns

- `collections.urn_suffix` is the full urn the node keeps: the provider urn
  (`third_party_id:urn_suffix`) for a linked collection; the collections-v2 urn
  of the contract for a published standard collection; NULL otherwise.
- `collections.contract_address`: the CREATE2 address upstream computed for
  every standard collection's draft, lowercased, as the node's schema
  requires (`collections_contract_address_lowercase_check`); NULL for a
  linked collection. `publication_pending` starts false: it marks the node's own
  publication in progress.
- `collections.lock`: the owner's last lock as upstream stores it; it holds
  for a day from that instant.
- `collections.linked_contract_address` (lowercased) and
  `linked_contract_network`: a third-party collection's linked contract, as
  upstream stores it.
- A collection's published and approved state is not imported. Upstream's
  `collections.is_published` and `is_approved` columns are not carried; the
  node derives both on read from its copy of the Polygon collections
  subgraph (by contract address), its own publication records and the items'
  `blockchain_item_id`.
- `items.is_published`: a blockchain item id in a published standard
  collection. Third-party items stay `false` (they have no blockchain id; the
  node's CHECK requires one): a third-party item is published once it has an
  item curation, which the `item_curation` step copies.
- `items.urn_suffix`: `urn:decentraland:matic:collections-v2:<contract>:<id>`
  for a published standard item, `third_party_id:collection:item` for a
  third-party item. The source is production builder-server, which runs on
  Polygon mainnet, so the URN network is always `matic` and the contract
  addresses are mainnet's. Import only into a node whose `ETH_NETWORK` is
  `mainnet`: the import can't see the target's network, and on an Amoy node
  (`ETH_NETWORK=sepolia`) the imported collections never read as published,
  because the Amoy copy holds none of their contracts, while their items
  keep answering `matic` URNs.
- `items.content_hash`: the newest item curation's content hash.
- `items.data` and `items.metrics` are copied as the JSON text upstream
  stores, keys in the order a save wrote them: the content hash of a
  published or third-party item serializes them in that order. `item`
  refuses a database whose `builder.items.data` is still `jsonb`: run
  `schemas/releases/builder-item-documents-and-uploaders.sql` first.
- Names are trimmed (node CHECK); empty `price`/`beneficiary`/`rarity` become
  NULL; addresses are lowercased. An item whose collection is missing
  upstream, or not on the node yet, becomes a loose draft (`collection_id`
  NULL); a later run attaches it once the node holds the collection (see
  "Re-runs").
- Timestamps: upstream `timestamp without time zone`, read as UTC.
- URNs follow the node's rules. A third-party collection's URN is taken
  when another third-party collection holds exactly that URN. A third-party
  item's URN is taken when another item in a collection of the same
  provider (`third_party_id`, compared exactly) holds the same item part,
  compared lowercased. A new or changed collection or item whose URN is
  taken, by a row on the node or another row of the same batch, is left
  out. Upstream deletions aren't detected (see "Re-runs"), so an item or
  collection upstream deleted and created again under its URN keeps its
  first copy on the node, and the new one is left out. Two rows that swapped
  URNs upstream are both left out. "Start over" carries both cases.

## Before the first run

The import writes only into a node whose builder rows are exactly what the
import left there. Run it while the node keeps creator writes closed
(`CREATE_WRITES_ENABLED=false`): every builder write answers 503 then, and
no builder job, operator console route or validator route writes either.
Repeat it as often as you need while writes stay closed, for rehearsals and
a final sync: each run refreshes the node from upstream. Then open writes.
From that point upstream is frozen for the node: the import refuses to run,
and edits made upstream later are not carried over.

Each step checks the node first, in the transaction of its first batch (and
before each step of a dry run). Every row of `builder.collections`,
`builder.items`, `builder.item_contents`, `builder.uploaded_contents`,
`builder.virtual_third_parties`, `builder.collection_curations`,
`builder.item_curations`, `builder.collection_events` and
`builder.slot_usage_cheques` must be recorded in
`dcl_migration.builder_imported`, and every recorded row must still exist
with the fingerprint the import recorded. Otherwise the step writes nothing
and stops with a message that names each table, its count of rows created,
changed or deleted on the node, and the first ids: the node has taken
builder writes and the import no longer applies to it. There is no
override. A node with no builder rows passes, as on a first run.

Run a release script or a schema step that rewrites builder rows before the
first import, not between runs: rows it changes stop matching their
fingerprints. The check reads every row of those tables: about 4 seconds at
the production size of builder-server.

Each step reports in its log what it left out, by count and first ids:

- A third-party collection or item whose URN another row holds: if
  upstream holds the duplicate, fix it upstream (or accept that the row
  stays out), then run again. If upstream holds no duplicate (two rows
  swapped URNs, or a row was deleted and created again under its URN),
  start over (see "Start over").
- An item whose `contents` names a file the node refuses: fix the name
  upstream (or accept that the item stays out), then run again (see
  "Blobs").
- A new item whose URN doesn't start with its collection's URN, or a changed
  one left as the node holds it: upstream holds an inconsistent item; fix it
  upstream or accept it.

## Virtual third parties

`virtual_third_party` copies each row of `virtual_third_parties`: the
provider id, its managers (lowercased; the node's CHECK requires it),
`raw_metadata` as upstream writes it (`tp:1:<name>:<description>[:<contracts>]`)
and `"isProgrammatic"` as `is_programmatic`. A row whose id is longer than
512 characters, the node's bound, is skipped. A re-run takes upstream's
current values (see "Re-runs"). Whether
a virtual third party's managers manage the provider is decided on read,
with the registry: see the builder's access rules.

## Curations, events, cheques and forum columns

The four curation steps run after `item` and copy only rows whose collection
or item is on the node, which the earlier steps imported; the others are
skipped. Counts are from
the reference dump of May 2025 (migrations through `1727096280687`).

| Rows | Rule | Count |
| --- | --- | --- |
| Collection curations | Import; assignee and reviewer lowercased, an assignee that is not an address and a reviewer that is neither an address nor `validator` become NULL, an empty reason list and a blank message become NULL | 4,755 of 4,755 |
| Collection curations of a collection the node lacks, or with a status outside builder-server's | Skip | 0 |
| Rejection reasons outside builder-server's twelve, or a message over 3,000 characters after trimming | Import the curation; clear the reasons or the message | 0 (the dump predates both columns; builder-server bounds both) |
| Item curations | Import; content hash lowercased | 38,211 of 38,337 |
| Item curations of one item created at the same instant | Keep the one updated last, then the highest id | 126 skipped (122 instants; 19 an untouched pending twin of an approved row) |
| Item curations of an item the node lacks, or whose content hash is not 64 hex digits | Skip | 0 |
| Collection events | Import; actor address lowercased, NULL when not an address | 0 (the dump predates the table) |
| Events of a collection the node lacks, or with a type, actor or payload outside builder-server's | Skip | 0 |
| Slot cheques | Import; salt and signature lowercased | 34 of 34 |
| Cheques of a collection the node lacks (builder-server keeps a deleted collection's cheques) | Skip | 0 |
| Cheques whose provider is not their collection's, whose salt or signature is malformed, whose quantity is not positive, or whose salt the provider already used | Skip | 0 |
| Forum links that are not an `http(s)` URL without spaces of at most 2,048 characters under the forum origin (`FORUM_URL`) | Clear, with the forum id | 5 (scripts, plain text and one foreign host; no forum id) |

Set `FORUM_URL` to the forum origin builder-server is deployed with (scheme
and host, no path or trailing slash, as its deployment sets it): the
`collection` step refuses to run without it and keeps a forum link only when
it starts with that origin and `/`.

The import never imports a forum claim timestamp.

A source older than builder-server's last two migrations has no review
columns on `collection_curations` and no `collection_events` table: the
extracts read the review columns when present and no events when the table
is absent. `updated_at` is never older than `created_at`. A re-run updates
a curation, never its collection, item or creation time (see "Re-runs");
events and cheques are never updated. A new curation created at the instant
of one already on the node is skipped, as is a cheque whose salt its
provider already used.

## Blobs

`builder.item_contents` keeps each file of an item's `contents` at its
position in the stored object, with `named_by`, the wallet that wrote the
name. `uploaded_contents` records, per hash and uploader, a blob the node
holds and its size. Upstream keeps neither, so the import attributes every
name an item holds to the item's owner and records that owner as an
uploader of every hash the item names, whoever first sent the bytes;
upstream served those files to anyone. A record of a hash an item names at
`video.mp4`, or as its `video`, is marked as a video the videos route took
(`uploaded_contents.video`), since upstream accepted it as one; publication
takes no other record for a `video.mp4` entry. `item_content` carries each
item's upstream `updated_at` and writes names and records only for items
whose node row still has it: an item upstream saved again after the `item`
walk gets its contents on the next run, and its hashes and video are not
checked against the blob inventory until then. An item's `video` is a name too: `item` writes
`video_named_by`, the owner; `item_content` records the owner as an uploader
of each staged item's video as well, and stops the batch when the video's
hash is not in the
blob inventory either. What an imported name and record let a wallet read is the file
routes' decision, the same as for a save and an upload on the node. The blob copy (out of
scope) copies the bytes into the blob store's item-files tree
(`<STORAGE_ROOT_FOLDER>/item-files`, where the node keeps item files apart from
the content tree) and must record each hash in `dcl_migration.blob_inventory (source,
object_key, hash, size)` with `source = 'builder-server'` and
`object_key = hash`; `item_content` stops the batch on any missing hash, and on any
hash whose inventory size is over 250,000,000 bytes, the largest file the node keeps
(`builder.uploaded_contents.size`; an item's video up to that size, any other file too).

Upstream accepts any file name in `contents`; the node refuses an empty
name, one over 256 characters and one with a backslash. An item whose
`contents` names such a file is left out whole, so the node never holds an
item whose files don't make up its content hash: `item` doesn't take that
save (the node keeps the item's earlier save, or doesn't hold the item),
and `item_content` writes none of its files and reports it by count and
first ids. Once the name is fixed upstream, the next run takes the item.

## Not migrated

- `projects`, `asset_packs`, `assets`, `deployments`, `pools`,
  `pool_groups`, `pool_likes`: no target. The node
  keeps no scenes.
- `collections.is_published`, `is_approved`: derived on the node, not stored.
- `collections.managers`, `minters`, `reviewed_at`: no target column.
- Terms of service: upstream forwards each acceptance to its warehouse and
  stores none, so there is nothing to import into `builder.terms_acceptances`.
- Left to the node: `collection_publications`,
  `items.in_catalyst`, `items.catalyst_content_hash`,
  `newsletter_subscriptions` (upstream forwards opt-ins and stores none).

## Re-runs

Every run re-scans upstream, with no watermark.
`dcl_migration.builder_imported (step, id, fingerprint)` records each row the
import wrote to the tables the check reads (see "Before the first run"),
with an md5 of the node row as the import left it: collections, items,
virtual third parties, curations, events and cheques by id, item contents by
item id and file, upload records by uploader and hash. A step records only
the rows it inserted or refreshed, in the batch's transaction.

The `collection`, `item`, `virtual_third_party` and curation steps treat an
upstream row as one they imported only when the ledger records it. A row the
node created under the same id, for example between two batches of a step
while writes were open, stops that batch on its key instead of taking
upstream's values, and the next run refuses the node. `item_content` upserts
names and upload records by key, so it relies on the check at the start of
its step.

On a later run, still with creator writes closed:

- An upstream row the node doesn't hold is inserted, as on the first run.
- A row the node holds takes upstream's current values when they differ,
  and its fingerprint is recorded again. A third-party collection that
  takes a new URN moves its items' URNs with it in the same transaction.
  This completes a collection or item an earlier run imported partly: an
  item whose upstream row moved before `item_content` ran reaches upstream's
  `updated_at`, so `item_content` then writes its contents, and an item
  imported loose because its collection wasn't on the node yet joins the
  collection.

Each step takes the node's locks (collections, then items, by id, and the
URN locks of a save) before it decides, so it waits for any transaction
holding them.

After any run, every third-party item's URN starts with its collection's URN
and `:`, no provider holds one item part twice (compared lowercased), and no
two third-party collections hold one URN.

A run stopped and resumed inside `item_content` resumes its walk after the
last item it finished, so an item that `item` inserted on the resumed run
with an id before that point gets its contents on the next run: run the
import once more after a resumed run.

Each fingerprint (`fingerprint.sql`) lists every column of its table, so a
column added to one of those tables joins its fingerprint in the same
change, and that change recomputes the recorded fingerprint of every row
whose other columns still match the one recorded: without it, every row
stops matching and the import refuses the node.

An item saved upstream between a run's `item` and `item_content` walks keeps
the contents it had (none, for an item new to that run) until the next run,
which brings the row and then its contents to the later save. Upstream
deletions, including files removed from an item's `contents`, are not
detected.

## Start over

A re-run carries what upstream changed in place, but not what upstream
deleted, and it checks a URN against the rows the node holds. So these stay
as the node holds them on every later run, each reported as left out
because another row holds its URN:

- Two third-party items of one provider that swapped item parts.
- Two third-party collections that swapped URNs.
- A collection or item deleted upstream and created again under its URN.
  The new collection's items are imported loose.

Make the final sync a start-over followed by a run, so the node equals
upstream whatever changed between rehearsals; start over earlier too
whenever a run leaves rows out with no duplicate upstream. Start over only
while creator writes have stayed closed since the first run, and don't run
the node's storage GC until the run that follows has finished: the copied
blobs are unreferenced in between. The run doesn't check the item-files tree
for them, so after a GC in that gap, delete the `builder-server` rows of
`dcl_migration.blob_inventory` and copy the blobs again before the run.

```sh
psql "$TARGET_DATABASE_URL" -X -f tooling/migration/sources/builder-server/start_over.sql
```

`start_over.sql` works in one transaction:

1. It refuses a node that still holds
   `builder.linked_collection_publications`, whose foreign key would stop
   the emptying: run `schemas/releases/builder-linked-publications-retired.sql`
   first.
2. It locks the tables it empties, waiting at most 10 seconds for a
   transaction that holds one and failing after that. A builder write that
   committed while it waited is seen by the check that follows; one that
   starts later waits for the start-over to end.
3. It runs the check from "Before the first run" and refuses, emptying
   nothing, a node that took builder writes. It also refuses a node holding
   a `builder.collection_publications` or `builder.validation_requests` row,
   which only a builder write makes.
4. It empties the nine tables the check reads, with
   `builder.collection_publications` and `builder.validation_requests`.
5. It deletes every row of `dcl_migration.builder_imported` and the
   `builder-server` rows of `dcl_migration.checkpoint`.

A node that ran an import that kept no ledger holds builder rows with no
`dcl_migration.builder_imported` row, so the check refuses every one of
them, and the refusal says so. If creator writes never opened on that node,
its rows are that import's, and it starts over with the check replaced by
one that the ledger is empty:

```sh
psql "$TARGET_DATABASE_URL" -X -v unrecorded_import=on \
  -f tooling/migration/sources/builder-server/start_over.sql
```

With any ledger row, `unrecorded_import` refuses and the node starts over
with the check.

The node then holds no builder rows, and the next `migrate.py run
builder-server` is a first run: it imports upstream as it is then. The blob
inventory stays, so the blobs need no second copy.
