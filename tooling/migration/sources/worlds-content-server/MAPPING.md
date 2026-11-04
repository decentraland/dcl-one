# worlds-content-server → `worlds`, `explore.world_scene_place`

Pinned at `decentraland/worlds-content-server@10af5255bb04` (migrations
`0002`..`0031`). Runs after `places`. The steps run in this order.

| Step | Upstream | Target | Rule |
| --- | --- | --- | --- |
| `world` | `worlds` | `worlds.worlds`, `world_permissions` (`access`), `world_access_communities` | One row per `lower(name)`. A lowercase spelling wins over a mixed-case one, otherwise the newest wins. The row is imported as local (`mirror_source` NULL). The `access` JSONB is split into `access_type`, `access_secret_hash` (the bcrypt hash, verified the same way by the node) and `access_nft`. A restricted setting with no secret or nft, or with an unknown type, becomes an empty `allow-list`, so it fails closed. `access.wallets` and `access.communities` (UUIDs only) replace the world's `access` grants and community rows. A `content_rating` outside `RP/E/T/A/R` becomes NULL; `categories` is capped at 20. |
| `world_scene` | `world_scenes` | `worlds.world_scenes` | A `DEPLOYED` row is imported as a local publish with its auth chain (JSON text unchanged), entity JSONB, deployer, parcels and size. An `UNDEPLOYED` row deletes the local target row. Mirrored scenes of an imported world are deleted, as a local publish deletes them. The identities of removed scenes are retired. |
| `world_scene_place` | `world_scenes` (`DEPLOYED`) | `explore.world_scene_place` | The `places` source has already bound most scenes exactly, through `places.deployment_id`. Those bindings are never changed here. For each imported scene still unbound, the identity is the retired upstream UUID the `places` source wrote for that world whose parcels overlap the scene, reactivated. That identity must not overlap any other local scene of the world. Otherwise the identity is `md5(world/entity)::uuid`. `world_scene` retires only the identities of the scenes it deletes. |
| `world_permission` | `world_permissions` + `world_permission_parcels` | `worlds.world_permissions`, `world_permission_parcels` | Covers the `deployment` and `streaming` grants. Addresses are lowercased; a value that is not an address is dropped. Each row carries the grant's canonical `x,y` parcel set and replaces the target set. An empty set means a world-wide grant. Upstream bumps the grant's `updated_at` on every parcel edit. |
| `quota_block` | `blocked` | `worlds.quota_blocks` | Lowercased, address values only. Upstream's `blocked` is the storage-quota block of an owner over its allowance, not a visitor ban, so it never lands in `worlds.blocked_wallets` (the node's platform-wide ban). |

`world` stores `thumbnail_hash` as the CID the blob copy stored the thumbnail
under, and `world_scene` needs every entity and content hash of a DEPLOYED
scene in the blob copy's records; see Blobs.

## Schema this mapping needs beyond `schemas/`

`worlds.quota_blocks (wallet text PRIMARY KEY, created_at timestamp NOT NULL,
updated_at timestamp NOT NULL)` and `worlds.world_limit_overrides` come with
the worlds storage-quota schema. On a schema without `worlds.quota_blocks`,
`quota_block` stops at the first non-empty batch rather than dropping the
blocks; `worlds-limits` refuses to run without `worlds.world_limit_overrides`.

## Blobs

`migrate.py blobs worlds-content-server` copies, before the row import, every
object the imported rows name: the entity file of each DEPLOYED scene (its
`entity_id`, the original signed bytes; `world_scenes.entity` is upstream's
JSONB, never the bytes to serve), each of its content files, and each world's
thumbnail. A world imported here is local, so neither the mirror nor
read-through fetches its files.

```sh
export WORLDS_CONTENT_SERVER_DATABASE_URL=postgres://...  TARGET_DATABASE_URL=postgres://...
python3 tooling/migration/migrate.py blobs worlds-content-server --from-dir /wcs/contents --to "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py blobs worlds-content-server --from-s3 wcs-bucket --to "$STORAGE_ROOT_FOLDER"
python3 tooling/migration/migrate.py blobs worlds-content-server --verify --to "$STORAGE_ROOT_FOLDER"   # after the import
```

- Upstream is a `STORAGE_FOLDER` (`<dir>/<sha1 shard>/<id>`, or flat with
  `disablePrefixHash`; a gzip-only `<id>.gzip` is inflated) or a `BUCKET`
  (key = id; `--from-s3` needs `boto3`, otherwise `aws s3 sync` the bucket and
  pass the copy as `--from-dir`).
- Every object is re-hashed before it is stored: a CIDv1 against
  `hash_bytes_v1` (raw leaf up to 256 KiB, the chunked dag-pb tree above), a
  CIDv0 against either upstream spelling (raw sha256, or `ipfs add`).
- Upstream's uploaded thumbnails are keyed by the sha256 hex of their bytes.
  They are stored under their `hash_bytes_v1` CID, and `world` rewrites
  `thumbnail_hash` to that CID, so a thumbnail of any size resolves.
  Deploy-derived thumbnails already are a scene content CID.
- The store layout is the node's: `<STORAGE_ROOT_FOLDER>/contents/<sha1
  shard>/<cid>`, written through a staging file and a rename. Run the copy as
  the user the node runs as.
- Each stored object is recorded in `dcl_migration.blob_inventory`
  (`object_key` = upstream key, `hash` = the CID). An object absent upstream,
  whose bytes do not hash to its key, or whose key is no content id is listed
  and recorded in `dcl_migration.blob_missing`; the node then serves it as
  missing, as upstream did. A thumbnail recorded there is imported as no
  thumbnail.
- Idempotent: objects already recorded and present in the store are skipped,
  so the copy runs again before each `run --since last`.
- `--verify` lists each imported local scene (and world thumbnail) whose
  blobs are not in the store. It checks existence; the copy checked content.

## Per-world limits

Upstream's per-world limit exceptions are not in its database: they are the
JSON document at its `WHITELIST_URL` (`{"<world>": {"max_parcels",
"max_size_in_mb", "allow_sdk6"}}`). Seed `worlds.world_limit_overrides` from a
copy of it:

```sh
python3 tooling/migration/migrate.py worlds-limits whitelist.json --max-size-mb <upstream MAX_SIZE>
```

Names are lowercased. `max_parcels` maps as is; `max_size_bytes` is
`max_size_in_mb` × 1 MiB. A listed world without `max_size_in_mb` had
upstream's fixed `MAX_SIZE` cap instead of its owner's allowance, so it gets
`--max-size-mb` × 1 MiB. ENS worlds (`.eth`, not `.dcl.eth`) keep the node's
ENS cap, as upstream ignored their listed size. `allow_sdk6` is not carried
over. Rows are upserted; a world removed from the document keeps its row.

## Not imported

- `UNDEPLOYED` scene rows (they only drive the deletes above), `pending_scenes`,
  `pending_scene_files`, `completed_scene_uploads`, `partial_upload_rates`
  (in-flight partial uploads) and `migrations`.
- The derived `worlds` columns: `last_deployed_at`, `deployed_scene_count`,
  `scene_min/max_x/y` and `search_vector`. Upstream's world `blockedSince` is
  derived from `blocked` joined on the owner. The node's `blocked_since` is an
  operator disable, which upstream does not have, so it is left alone, as are
  `realm_name_override` and `preview_wearable_urns`.
- Removed deployment/streaming grants and lifted quota blocks are not
  detected.

## Left to the node

- `worlds.remote_worlds` and `mirror_passes`: catalogue mirror state. The
  catalogue pass rebuilds them, and it skips local worlds.
- `federation_worlds` and `federation_peer_status`: claims made by peers,
  rebuilt by polling.
- `forward_outbox`: upstream forwarding of new local publishes.
- `world_access_log`: upstream has no such table.
- `worlds.world_scene_identity`: a frozen upgrade source. It is never written.
