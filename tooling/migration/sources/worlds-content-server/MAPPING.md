# worlds-content-server → `worlds`, `explore.world_scene_place`

Pinned at `decentraland/worlds-content-server@10af5255bb04` (migrations
`0002`..`0031`). Runs after `places`. The steps run in this order.

| Step | Upstream | Target | Rule |
| --- | --- | --- | --- |
| `world` | `worlds` | `worlds.worlds`, `world_permissions` (`access`), `world_access_communities` | One row per `lower(name)`. A lowercase spelling wins over a mixed-case one, otherwise the newest wins. The row is imported as local (`mirror_source` NULL). The `access` JSONB is split into `access_type`, `access_secret_hash` (the bcrypt hash, verified the same way by the node) and `access_nft`. A restricted setting with no secret or nft, or with an unknown type, becomes an empty `allow-list`, so it fails closed. `access.wallets` and `access.communities` (UUIDs only) replace the world's `access` grants and community rows. A `content_rating` outside `RP/E/T/A/R` becomes NULL; `categories` is capped at 20. |
| `world_scene` | `world_scenes` | `worlds.world_scenes` | A `DEPLOYED` row is imported as a local publish with its auth chain (JSON text unchanged), entity JSONB, deployer, parcels and size. An `UNDEPLOYED` row deletes the local target row. Mirrored scenes of an imported world are deleted, as a local publish deletes them. The identities of removed scenes are retired. |
| `world_scene_place` | `world_scenes` (`DEPLOYED`) | `explore.world_scene_place` | The `places` source has already bound most scenes exactly, through `places.deployment_id`. Those bindings are never changed here. For each imported scene still unbound, the identity is the retired upstream UUID the `places` source wrote for that world whose parcels overlap the scene, reactivated. That identity must not overlap any other local scene of the world. Otherwise the identity is `md5(world/entity)::uuid`. `world_scene` retires only the identities of the scenes it deletes. |
| `world_permission` | `world_permissions` + `world_permission_parcels` | `worlds.world_permissions`, `world_permission_parcels` | Covers the `deployment` and `streaming` grants. Addresses are lowercased; a value that is not an address is dropped. Each row carries the grant's canonical `x,y` parcel set and replaces the target set. An empty source set means a world-wide grant. Coordinates cover the complete signed 32-bit range; any malformed or out-of-range restriction aborts the batch before changing grants. Upstream bumps the grant's `updated_at` on every parcel edit. |
| `quota_block` | `blocked` | `worlds.quota_blocks` | Lowercased, address values only. Upstream's `blocked` is the storage-quota block of an owner over its allowance, not a visitor ban, so it never lands in `worlds.blocked_wallets` (the node's platform-wide ban). |

### Targets loaded by the former `blocked_wallet` step

An earlier mapping loaded `blocked` into `worlds.blocked_wallets` as the
step `blocked_wallet`, so those owners are banned platform-wide. Undo it once
on such a target:

```sh
python3 tooling/migration/migrate.py worlds-unban-quota-blocks --dry-run
python3 tooling/migration/migrate.py worlds-unban-quota-blocks
```

It runs only when that step's checkpoint exists, and drops the checkpoint in
the same transaction. The tool keeps no per-wallet provenance, so it deletes
the `blocked_wallets` rows whose wallet upstream `blocked` holds with the same
`created_at` (the step copied it verbatim; a node ban has its own) and whose
`updated_at` is no later than the start of that step's last walk (banning a
wallet again on the node keeps `created_at` and bumps `updated_at`). It lists
the rows it kept for either reason. A ban made on the node before the old
step ran had both timestamps overwritten by it and is deleted. Risk: a
wallet whose upstream block was lifted after the old step ran is not in
`blocked` any more and stays banned; review `worlds.blocked_wallets` by hand.

`world` stores `thumbnail_hash` as the CID the blob copy stored the thumbnail
under, and `world_scene` needs every entity and content hash of a DEPLOYED
scene in the blob copy's records; see Blobs.

## Schema this mapping needs beyond `schemas/`

`worlds.quota_blocks (wallet text PRIMARY KEY, created_at timestamp NOT NULL,
updated_at timestamp NOT NULL)` and `worlds.world_limit_overrides` come with
the worlds storage-quota schema. On a schema without `worlds.quota_blocks`,
`quota_block` stops at the first non-empty batch rather than dropping the
blocks. A block the node already holds keeps the earlier `created_at` (its
grace window does not restart) and the later `updated_at`; `worlds-limits` refuses to run without `worlds.world_limit_overrides`.

## Blobs

`migrate.py blobs worlds-content-server` copies, before the row import, every
object the imported rows name: the entity file of each DEPLOYED scene (its
`entity_id`, the original signed bytes; `world_scenes.entity` is upstream's
JSONB, never the bytes to serve), each of its content files, and each world's
thumbnail. A world imported here is local, so neither the mirror nor
read-through fetches its files.

```sh
export WORLDS_CONTENT_SERVER_DATABASE_URL=postgres://...  TARGET_DATABASE_URL=postgres://...
export STORAGE_ROOT_FOLDER=...                         # the node's storage root
python3 tooling/migration/migrate.py blobs worlds-content-server --from-dir /wcs/contents
python3 tooling/migration/migrate.py blobs worlds-content-server --from-s3 wcs-bucket
python3 tooling/migration/migrate.py run worlds-content-server            # ends with the --verify check
python3 tooling/migration/migrate.py blobs worlds-content-server --verify  # the same check on its own
```

**Do not run the node's storage GC (`catalyst storage gc`, worlds
`/admin/gc`) between `blobs` and `run`.** Copied blobs are not referenced
until `run` imports their rows, and the GC's age floor (one hour) does not
know the migration inventory, so it deletes them; the loads check only the
inventory tables. A `run` that loads `world` or `world_scene` therefore ends
by checking that the store holds every blob an imported local scene or world
thumbnail names, and fails (exit 1, listing them) when one is missing: run
`blobs` again, then `run` again. It needs the node's storage root
(`--storage-root` or `STORAGE_ROOT_FOLDER`).

- Upstream is a `STORAGE_FOLDER` (`<dir>/<sha1 shard>/<id>`, or flat with
  `disablePrefixHash`; a gzip-only `<id>.gzip` is inflated) or a `BUCKET`
  (key = id; `--from-s3` needs `boto3`, otherwise `aws s3 sync` the bucket and
  pass the copy as `--from-dir`). A 404 is recorded as missing. A 403 is
  recorded as missing (a missing key reads as 403 without `s3:ListBucket`)
  once a probed key has read; before that it stops the copy. A read that
  fails part-way (timeout, truncated stream), S3 throttling (`SlowDown`) or a
  5xx is retried three times with backoff; a key still failing is listed, not
  recorded, and the copy exits 1 so the next run retries it. Any other S3
  error stops the copy, `RequestTimeTooSkewed` (a wrong system clock)
  included.
- Keys recorded in `dcl_migration.blob_missing` by an earlier run are skipped;
  `--retry-missing` retries them (one that reads is stored and leaves
  `blob_missing`). They never count toward the checks below.
- Before copying, up to 16 new keys spread over the work list and up to 4 keys
  an earlier run copied are opened. When none reads (a wrong `--from-dir` or
  bucket, or credentials that cannot read it) the copy stops; `--skip-probe`
  copies anyway. When more of the new keys are absent or unreadable upstream
  than both `--max-missing` percent (default 1) and `--max-missing-count`
  (default 10), the copy exits 1 and records none of them missing.
  `--allow-missing` skips that count, not the probe.
- At most `--workers` × 4 keys are in flight at a time.
- Every object is re-hashed before it is stored: a CIDv1 against
  `hash_bytes_v1` (raw leaf up to 256 KiB, the chunked dag-pb tree above), a
  CIDv0 against either upstream spelling (raw sha256, or `ipfs add`).
- Upstream's uploaded thumbnails are keyed by the sha256 hex of their bytes.
  They are stored under their `hash_bytes_v1` CID, and `world` rewrites
  `thumbnail_hash` to that CID, so a thumbnail of any size resolves.
  Deploy-derived thumbnails already are a scene content CID.
- The store layout is the node's: `<STORAGE_ROOT_FOLDER>/contents/<sha1
  shard>/<cid>`, written through a new staging file (`O_EXCL|O_NOFOLLOW`, as
  the node) and a rename; the staging file is removed when the read fails. Run
  the copy as the user the node runs as.
- Each stored object is recorded in `dcl_migration.blob_inventory`
  (`object_key` = upstream key, `hash` = the CID). An object absent upstream,
  whose bytes do not hash to its key, or whose key is no content id (keys are
  matched whole, so one holding a line break is invalid) is listed and
  recorded in `dcl_migration.blob_missing`; the node then serves it as
  missing, as upstream did. A thumbnail recorded there is imported as no
  thumbnail.
- Idempotent: objects already recorded and present in the store with their
  recorded size are skipped, so the copy runs again before each
  `run --since last`. A file the store already holds for an unrecorded key is
  re-hashed, and copied again when its bytes do not match.
- `--verify` lists each imported local scene (and world thumbnail) whose
  blobs are not in the store. It checks existence; the copy checked content.
- Places: upstream Places stored an uploaded world thumbnail as the image URL
  `<worlds server>/contents/<sha256 hex>`. The `places` source's `place` step
  rewrites the hex to the recorded CID, so run this copy before `run places`;
  a `places` run before it leaves those URLs (it prints a notice), and the
  step re-scans fully, so running it again after the copy fixes them.

## Per-world limits

Upstream's per-world limit exceptions are not in its database: they are the
JSON document at its `WHITELIST_URL` (`{"<world>": {"max_parcels",
"max_size_in_mb", "allow_sdk6"}}`). Seed `worlds.world_limit_overrides` from a
copy of it:

```sh
python3 tooling/migration/migrate.py worlds-limits whitelist.json --max-size-mb <upstream MAX_SIZE>
```

Upstream looks an entry up by the lowercased world name, so only keys that
are already lowercase and trimmed ever applied; others are skipped with a
warning. A limit may be a number or an integer string (upstream coerces
both); 0 or anything else means the default. `max_parcels` maps as is; `max_size_bytes` is
`max_size_in_mb` × 1 MiB. A listed world without `max_size_in_mb` had
upstream's fixed `MAX_SIZE` cap instead of its owner's allowance, so it gets
`--max-size-mb` × 1 MiB. ENS worlds (`.eth`, not `.dcl.eth`) keep the node's
ENS cap, as upstream ignored their listed size. `allow_sdk6` is not carried
over. A world without a row gets one. An existing row is updated only while
it still holds the values this command last wrote
(`dcl_migration.world_limit_seeded`); a row set or changed on the node is kept
and listed. A world removed from the document keeps its row. Limits beyond
the columns' range are capped (`max_parcels` at 2^31-1).

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

## Blocked names: required cutover preflight

Export the running source deployment's `BLOCKED_WORLD_NAMES` setting to a JSON
array file and pass `--worlds-blocked-names FILE` for worlds runs (including dry
runs). An explicitly confirmed empty array `[]` is accepted; a missing file or
nonempty list stops the migration. Do not infer an empty list from the database:
this restriction is deployment configuration, not a table.

The source blocks exact world names. The target's shared banned-name list removes
`.eth`/`.dcl` when checking names, so copying this list could also block an
unrelated sibling name. Setting `blocked_since` on existing worlds would miss
blocked names without rows and does not establish equivalent admission checks.
For a nonempty list, retain the current service owner until an equivalent target
exact-name policy is implemented and verified across deployment and read routes.
