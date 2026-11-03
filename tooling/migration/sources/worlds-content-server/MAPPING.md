# worlds-content-server → `worlds`, `explore.world_scene_place`

Pinned at `decentraland/worlds-content-server@10af5255bb04` (migrations
`0002`..`0031`). Runs after `places`. The steps run in this order.

| Step | Upstream | Target | Rule |
| --- | --- | --- | --- |
| `world` | `worlds` | `worlds.worlds`, `world_permissions` (`access`), `world_access_communities` | One row per `lower(name)`. A lowercase spelling wins over a mixed-case one, otherwise the newest wins. The row is imported as local (`mirror_source` NULL). The `access` JSONB is split into `access_type`, `access_secret_hash` (the bcrypt hash, verified the same way by the node) and `access_nft`. A restricted setting with no secret or nft, or with an unknown type, becomes an empty `allow-list`, so it fails closed. `access.wallets` and `access.communities` (UUIDs only) replace the world's `access` grants and community rows. A `content_rating` outside `RP/E/T/A/R` becomes NULL; `categories` is capped at 20. |
| `world_scene` | `world_scenes` | `worlds.world_scenes` | A `DEPLOYED` row is imported as a local publish with its auth chain (JSON text unchanged), entity JSONB, deployer, parcels and size. An `UNDEPLOYED` row deletes the local target row. Mirrored scenes of an imported world are deleted, as a local publish deletes them. The identities of removed scenes are retired. |
| `world_scene_place` | `world_scenes` (`DEPLOYED`) | `explore.world_scene_place` | The `places` source has already bound most scenes exactly, through `places.deployment_id`. Those bindings are never changed here. For each imported scene still unbound, the identity is the retired upstream UUID the `places` source wrote for that world whose parcels overlap the scene, reactivated. That identity must not overlap any other local scene of the world. Otherwise the identity is `md5(world/entity)::uuid`. `world_scene` retires only the identities of the scenes it deletes. |
| `world_permission` | `world_permissions` + `world_permission_parcels` | `worlds.world_permissions`, `world_permission_parcels` | Covers the `deployment` and `streaming` grants. Addresses are lowercased; a value that is not an address is dropped. Each row carries the grant's canonical `x,y` parcel set and replaces the target set. An empty set means a world-wide grant. Upstream bumps the grant's `updated_at` on every parcel edit. |
| `blocked_wallet` | `blocked` | `worlds.blocked_wallets` | Lowercased, address values only. |

## Not imported

- `UNDEPLOYED` scene rows (they only drive the deletes above), `pending_scenes`,
  `pending_scene_files`, `completed_scene_uploads`, `partial_upload_rates`
  (in-flight partial uploads) and `migrations`.
- The derived `worlds` columns: `last_deployed_at`, `deployed_scene_count`,
  `scene_min/max_x/y` and `search_vector`. Upstream's world `blockedSince` is
  derived from `blocked` joined on the owner. The node's `blocked_since` is an
  operator disable, which upstream does not have, so it is left alone, as are
  `realm_name_override` and `preview_wearable_urns`.
- Removed deployment/streaming grants and unblocked wallets are not detected.

## Left to the node

- **Blobs.** The entity file of each scene (under its `entity_id`, as the
  original signed bytes) and every content file must be copied into the blob
  store before cutover. `world_scenes.entity` is upstream's JSONB, never the
  bytes to serve. A world imported here is local, so neither the mirror nor
  read-through will fetch its files.
- `worlds.remote_worlds` and `mirror_passes`: catalogue mirror state. The
  catalogue pass rebuilds them, and it skips local worlds.
- `federation_worlds` and `federation_peer_status`: claims made by peers,
  rebuilt by polling.
- `forward_outbox`: upstream forwarding of new local publishes.
- `world_access_log`: upstream has no such table.
- `worlds.world_scene_identity`: a frozen upgrade source. It is never written.
