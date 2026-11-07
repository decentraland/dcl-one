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
| `main`, active Genesis place in `explore.place` | `main` | the place's canonical base parcel |
| `main`, disabled or unknown place | `main` | `legacy:<place_id>`, retained but unreachable |

A Places UUID that no imported scene is bound to (a retired place) stays as
an unknown legacy scope of its world. It still counts toward the world
quota, as it did upstream. Rows that collapse onto one target key keep the
newest. A target row newer than the source row is never overwritten.

- `world_storage` and `player_storage`: `value` and `value_size` are copied.
  Player addresses are lowercased; a row whose player is not an address is
  skipped.
- `env_variables`: `value_enc` is copied byte for byte. Upstream and the node
  use the same AES-256-GCM layout (`0x01 || 12-byte IV || ciphertext ||
  16-byte tag`, no AAD). The node must therefore run with upstream's
  `ENCRYPTION_KEY`. SQL cannot re-encrypt the values (pgcrypto has no GCM),
  and no key or plaintext passes through this tool.

Not imported: `scene_collaborators` (the node derives collaborators at read
time from `metadata.logsPermissions`) and `pgmigrations`. Values deleted
upstream are not detected.
