# world-storage-service → `worlds.world_storage`, `player_storage`, `env_variables`

Pinned at `decentraland/world-storage-service@d55f24cd52ac`. Runs after
`worlds-content-server`, which runs after `places`.

Upstream keys every row by `(world_name, place_id)`. `world_name` is the
lowercase world, or `main` for any Genesis City realm. `place_id` is the
Places UUID that the request resolved; it is the nil UUID where the
`1775047654810` backfill found none. The node keys rows by
`(realm, scene_namespace)`:

| Upstream | `realm` | `scene_namespace` |
| --- | --- | --- |
| `*.eth` world, any `place_id` | world name | `place_id`, which is the identity `worlds-content-server/world_scene_place` binds to the scene standing on that place |
| `*.eth` world, nil `place_id` | world name | `world`, the parked legacy bucket |
| `main`, active Genesis place in `explore.place` | `main` | the effective base parcel of the place's scene (below) |
| `main`, disabled or unknown place | `main` | `legacy:<place_id>`, retained but unreachable |

A Genesis scene's runtime namespace is its effective base
(`ParcelSet::effective_base` in `services/catalyst/comms/src/world_identity.rs`):
the declared `metadata.scene.base` when it is one of the scene's parcels,
otherwise the scene's least parcel by `x`, then `y`, in canonical `x,y` form.
Places' `base_position` is the declared base, so the import does not use it
as is. The scene is the active content deployment (`content.active_pointers`)
standing on one of the place's parcels and declaring the same base as the
place; without one (content not yet synced, or the place is stale), the
place's own `positions`, or `base_position` alone when it lists none. The
rule is the same SQL in each of the three loads.

A world place UUID that no active scene is bound to (a scene undeployed, or
redeployed into a new place) keeps its rows. When `worlds-content-server`
seeded that UUID as an inactive `world_scene_place`, those rows are retained
but excluded from the world quota and `/usage`; a UUID with no
`world_scene_place` record still counts, and so does the parked `world`
bucket until the world has an active identity. Rows that collapse onto one
target key keep the newest. A target row newer than the source row is never
overwritten.

- `world_storage` and `player_storage`: `value` and `value_size` are copied.
  Player addresses are lowercased; a row whose player is not an address is
  skipped.
- `env_variables`: `value_enc` is copied byte for byte. Upstream and the node
  use the same AES-256-GCM layout (`0x01 || 12-byte IV || ciphertext ||
  16-byte tag`, no AAD). The node must therefore run with upstream's
  `ENCRYPTION_KEY`. SQL cannot re-encrypt the values (pgcrypto has no GCM),
  and no key or plaintext passes through this tool.

## Deletions

Each load records in `dcl_migration.wss_<step>_loaded` every upstream key
whose target row it wrote, with that row's scope and `updated_at`. After
each load, its `<step>_reconcile` step (`whole`) reads upstream's full key
set and deletes a recorded row whose key upstream no longer holds, when:

- the target row's `updated_at` is still the recorded one (a node write since
  sets its own and keeps the row), and
- no other recorded upstream key still lands on that target row.

A row the import never recorded (written on the node, or kept because it was
newer than upstream) is never deleted, and neither is a retired scope's row
upstream still holds. The reconcile refuses to run when upstream lists no
keys but imported ones are recorded. Its key set must fit one transaction.

Limits: when the newest of several upstream keys collapsed onto one target
row is deleted and an older one remains, the row keeps the deleted value
until that key changes upstream. A key re-scoped between runs (its place
disabled or re-based) moves to the new scope when it is re-imported; the copy
at the old scope stays. Rows imported before these records existed are
recorded by `run world-storage-service --restart`.

Not imported: `scene_collaborators` (the node derives collaborators at read
time from `metadata.logsPermissions`) and `pgmigrations`.
