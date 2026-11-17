# places → catalyst

Upstream: `github.com/decentraland/places` at the `revision` in `source.toml`
(final table shapes from every `src/migrations/*` up to
`1790812800000_add-removal-clock-to-world-watermarks`). Every upstream
timestamp column used here is `timestamp without time zone` holding UTC.

## Authority

The import makes this node the authority for the catalogue. A row looks like
the node's own write paths' rows, not a mirror's:

- `explore.place.origin = 'local'`, `raw.local = true`, `raw.source = 'places'`
  (neither `'content'` nor `'worlds'`, so the derive pass neither prunes nor
  rewrites it). The places mirror never sweeps it.
- `likes`, `dislikes`, `favorites` are 0: the facts are imported into
  `user_likes`/`user_favorites` and counted in `place_interaction_counts`,
  which the reads add on top (upstream's counters are exactly these counts).
- Operator decisions become the overlays the mirror and derive re-apply:
  `place_moderation` (moderation disable, highlight, a standing moderated
  rating) and `place_ranking` (`ranking` ≠ 0, `exclude_from_ranking`), with
  `set_by = 'migration:places'`. A decision taken on the node (any other
  `set_by`) is never overwritten and wins over the imported row's fields.

Run with `PLACES_MIRROR_UPSTREAM` unset. A mirror pass upserts every id
upstream still lists back to `origin = 'upstream'` with upstream's counters,
which then count twice with the imported facts. Rows upstream does not list
(opt-out, moderation-disabled) would stay local.

## Steps

| Step | Source | Target | Key | Watermark |
| --- | --- | --- | --- | --- |
| `place` | `places` (`world IS FALSE`, enabled or `disabled_reason` in `opt_out`, `moderation`); `worlds` with an enabled scene, as `/api/worlds` lists them; `content_ratings` (latest, only when it is still the place's rating) | `explore.place`, `explore.place_moderation`, `explore.place_ranking` | `id` | none (upstream does not bump `updated_at` on every write) |
| `world_scene_place` | `places` (`world IS TRUE`) | `explore.world_scene_place` (identity = place UUID) | `identity` | none |
| `user_favorite` | `user_favorites` | `explore.user_favorites`, `explore.place_interaction_counts` | `user`, `entity_id` | `created_at` |
| `user_like` | `user_likes` | `explore.user_likes`, `explore.place_interaction_counts` | `user`, `entity_id` | `updated_at` |
| `road_position` | `road_positions` | `explore.road_positions` (adds only parcels the schema seed lacks) | `position` | none |

- World rows keep upstream's id (the lowercase world name). A world or id
  already in `explore.place_world_local` is skipped. When the worlds import
  and the derive pass publish a world locally, derive replaces the imported
  row with its local-leg row under the same id, and re-applies the overlays.
- A world image of the form `<worlds server>/contents/<sha256 hex>` (an
  uploaded thumbnail) is rewritten to end in the CID
  `migrate.py blobs worlds-content-server` recorded for that key in
  `dcl_migration.blob_inventory`, the only name the node serves it under. Run
  that copy before this step; a key it has not recorded is left as is with a
  notice, and re-running this step (a full re-scan) after the copy fixes it.
- World-scene identities: a row is active (bound to `deployment_id`) when it
  has a deployment id, is enabled or opted out, and no preferred row of the
  same world (enabled first, then newest) overlaps it. All others are kept as
  retired identities, so storage keyed on them stays addressable. A binding
  another identity already holds on the node stays with that identity.
  Re-runs move an identity only if the node has not updated it since.
- Favorites and likes: wallets lowercased, non-wallet users skipped; kept only
  for an imported place or any upstream world name. A like changed later on
  the node wins. Each batch recounts its entities from the fact tables
  (`REBUILD_COUNTS` rule).
- `contact_email` and the other public scene metadata travel in `raw`, as the
  upstream API serves them.

## Not migrated

- Disabled `overwritten` / `undeployment` places: replaced history. Their
  likes and favorites go too, as the mirror's sweep would delete them.
- World-scene places as catalogue rows. The node's catalogue lists the world
  aggregates only; scenes are identities.
- Worlds with no enabled scene: upstream does not list them.
- `categories` (static on the node), `place_positions` (index of
  `positions`), `entities_places`, `check_scenes_logs`, `tasks`,
  `world_undeployments`, `world_scene_undeployments`,
  `world_deployment_position_watermarks` (upstream ingestion state), `users`
  (admins come from `PLACES_ADMIN_ADDRESSES`), and the `content_ratings`
  history.
- `explore.moderators` is written by the events import (events moderators).
  Upstream places has no report table, so `explore.place_reports` gets nothing.

## Left to the node

- `explore.place_removal_pending` / `place_removal_state`: the catalogue
  triggers fill them. A moderation-disabled place enqueues one removal, as
  the schema's own seed does for an existing disabled catalogue.
- `explore.place_indexed` (view), `mirror_state`, `signed_actions`,
  `seen_nonces`.

## Deletes and limits

The `place` load deletes only overlays it wrote whose upstream decision is
gone. Rows that disappear upstream are not detected. That includes
unfavorites, a world losing its last scene, and a place becoming
overwritten. Run the final pass with upstream writes stopped.
