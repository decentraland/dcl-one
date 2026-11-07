# builder-server -> builder

Pinned at `452af1e06cf103ffa4e5b115b002120e98d375c7` (migrations through
`1727096280687_add-programmatic-check-to-third-parties`).

| Step | Upstream | Target |
| --- | --- | --- |
| `collection` | `collections` | `builder.collections` |
| `item` | `items` + newest `item_curations` + parent `collections` | `builder.items` |
| `item_content` | `items.contents` (file -> hash) | `builder.uploaded_contents`, `builder.item_contents` |
| `project` | `projects` | `builder.projects` |

## Derived columns

- `collections.urn_suffix` is the full urn the node keeps: the provider urn
  (`third_party_id:urn_suffix`) for a linked collection; the collections-v2 urn
  of the contract for a published standard collection; NULL otherwise.
- `collections.contract_address` only for a published standard collection
  (upstream stores the CREATE2 address from the draft on; the node's CHECK
  ties a contract to publication). `publication_pending` starts false: the
  upstream `lock` is a short-lived publish lock.
- `items.is_published`: a blockchain item id in a published standard
  collection. Third-party items stay `false` (they have no blockchain id; the
  node's CHECK requires one) and carry their state in `curation_status`.
- `items.urn_suffix`: `urn:decentraland:matic:collections-v2:<contract>:<id>`
  for a published standard item, `third_party_id:collection:item` for a
  third-party item.
- `items.curation_status`: newest `item_curations.status`; else `approved` for
  a published item in an approved collection; else `pending`. `is_approved` is
  its projection. `content_hash` is the newest curation's content hash.
- Names are trimmed (node CHECK); empty `price`/`beneficiary`/`rarity` become
  NULL; addresses are lowercased. An item whose collection is missing
  upstream becomes a loose draft (`collection_id` NULL).
- Timestamps: upstream `timestamp without time zone`, read as UTC.
- `projects`: rows without `eth_address` (ownerless legacy rows) are skipped;
  NULL statistics become 0, NULL description `''`; `thumbnail` is not carried
  (upstream holds a file name in the project's S3 folder, the node an upload
  hash). `files_version`/`manifest_revision` keep the node's values.

## Blobs

`builder.item_contents.hash` is an FK to `uploaded_contents`, which records a
blob the node holds and its size. The blob copy (out of scope) must record
each hash in `dcl_migration.blob_inventory (source, object_key, hash, size)`
with `source = 'builder-server'` and `object_key = hash`; `item_content`
stops the batch on any missing hash. `uploaded_by` is the item owner.

## Not migrated

- `asset_packs`, `assets`: upstream packs belong to a wallet, the node's to a
  project (`asset_packs.project_id NOT NULL`); the default packs are
  `builder.asset_catalog`, built by the operator command.
- Project manifests and files: S3 objects, not rows (`project_manifests`,
  `project_files` stay empty for imported projects).
- `collection_curations`, `slot_usage_cheques`, `virtual_third_parties`,
  `deployments`, `pools`, `pool_groups`, `pool_likes`: no target.
- `collections.managers`, `minters`, `reviewed_at`, `forum_link`, `forum_id`,
  `lock`, `linked_contract_address`, `linked_contract_network`: no target column.
- Left to the node: `collection_publications`, `linked_collection_publications`,
  `items.in_catalyst`, `items.catalyst_content_hash`, `asset_catalog`,
  `newsletter_subscriptions` (upstream forwards opt-ins and stores none).

No watermarks (state is derived across tables); every run re-scans. Upstream
deletions, including files removed from an item's `contents`, are not detected.
