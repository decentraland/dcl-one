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
  every standard collection's draft, lowercased; NULL for a linked
  collection. `publication_pending` starts false: it marks the node's own
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
  third-party item.
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
- A re-run updates a collection or item only while its unit, the
  collection with its items and their curations, is as the import left it
  (see "Re-runs").
- A collection or item id the node holds with another creation time is a
  row the import didn't write, such as an id a wallet took on the node
  before the import: `collection` and `item` stop the batch and name the
  first ten such ids for the operator, attaching nothing to them.

## Virtual third parties

`virtual_third_party` copies each row of `virtual_third_parties`: the
provider id, its managers (lowercased; the node's CHECK requires it),
`raw_metadata` as upstream writes it (`tp:1:<name>:<description>[:<contracts>]`)
and `"isProgrammatic"` as `is_programmatic`. A row whose id is longer than
512 characters, the node's bound, is skipped. A re-run updates a row only
while the node row is as the import left it (see "Re-runs"). Whether
a virtual third party's managers manage the provider is decided on read,
with the registry: see the builder's access rules.

## Curations, events, cheques and forum columns

The four curation steps run after `item` and copy only rows whose collection
or item the earlier steps imported; the others are skipped. Counts are from
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
| Cheques of a collection the node lacks (builder-server keeps a deleted collection's cheques) | Skip; a cheque of a collection the node deletes stays with no collection | 0 |
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
a curation, never its collection, item or creation time, only while its unit
is as the import left it (see "Re-runs"); events and cheques are never
updated.

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
whose node row still has it: an item the node saved since keeps its own
contents on re-runs, and its hashes and video are not checked against the
blob inventory. An item's `video` is a name too: `item` writes
`video_named_by`, the owner; `item_content` records the owner as an uploader
of each staged item's video as well, and stops the batch when the video's
hash is not in the
blob inventory either. What an imported name and record let a wallet read is the file
routes' decision, the same as for a save and an upload on the node. The blob copy (out of
scope) copies the bytes into the blob store's builder tree
(`<STORAGE_ROOT_FOLDER>/builder`, where the node keeps item files apart from
the content tree) and must record each hash in `dcl_migration.blob_inventory (source,
object_key, hash, size)` with `source = 'builder-server'` and
`object_key = hash`; `item_content` stops the batch on any missing hash, and on any
hash whose inventory size is over 250,000,000 bytes, the largest file the node keeps
(`builder.uploaded_contents.size`; an item's video up to that size, any other file too).

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
`dcl_migration.builder_imported (step, id, fingerprint)` records each
collection, item, virtual third party, collection curation and item curation
the import wrote, with an md5 of every column of the node row as the import
left it.

A collection, its items, its collection curations and its items' curations
form one unit; a loose item and its curations form one too. A unit is the
import's while every one of its rows on the node is recorded with its
fingerprint; a row the node changed, whether or not the change moved
`updated_at` (a forum topic or a claim on one, a publication in progress, a
pinned video, a linked collection's new URN, a refreshed curation hash), a row
the node created in it, or a row recorded without a fingerprint makes the unit
the node's. On a later run:

- An id the table doesn't record is inserted, as on the first run, with two
  exceptions. A new third-party item whose URN doesn't start with its
  collection's URN on the node is left out. A new curation of a unit the node
  holds is left out, because the newest curation decides the review state the
  node owns.
- The rows of a unit that is still the import's take upstream's current
  values when they differ, and the table records their new fingerprints. A
  third-party collection that takes a new URN moves its items' URNs with it in
  the same transaction. This completes a unit an earlier run imported partly:
  an item whose upstream row moved before `item_content` ran reaches
  upstream's `updated_at`, so `item_content` then writes its contents, and an
  item imported loose because its collection wasn't on the node yet joins the
  collection.
- A unit the node holds keeps every one of its rows as the node has them.
- A recorded row the node deleted stays deleted, and doesn't make its unit
  the node's.

Each step decides with the unit's collections and items locked in the
node's order (collections, then items, by id), so a node save waits for the
step's transaction or the step sees the save. `item_content` takes the same
locks before it chooses the items still at upstream's time. Each step reports
in its log the rows it left out, by count and first ids.

After any run, every third-party item's URN starts with its collection's URN
and `:`.

Each fingerprint lists every column of its table, so a column added to
`builder.collections`, `builder.items`, `builder.virtual_third_parties`,
`builder.collection_curations` or `builder.item_curations` joins its
fingerprint in the same change. The `collection`, `item`,
`collection_curation` and `item_curation` loads each carry the same copy of
the four unit fingerprints and the unit rule, which a migration test holds
identical.

An item saved upstream between a run's `item` and `item_content` walks keeps
the contents it had (none, for an item new to that run) until the next run,
which brings the row and then its contents to the later save. Upstream deletions, including files removed from
an item's `contents`, are not detected.
