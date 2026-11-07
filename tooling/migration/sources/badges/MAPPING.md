# badges → social

Upstream: `processor/src/migrations` at the pinned revision: the badge
catalog (`badge_definitions`, `badge_tiers`, 34 badges and 85 tiers since
migration 1791331200000) and `user_progress` (one row per wallet and badge:
`progress` document, `achieved_tiers` `[{tier_id, completed_at}]`, epoch-ms
`completed_at` and `updated_at`).

| Step | Upstream | Target |
| --- | --- | --- |
| `catalog` | `badge_definitions`, `badge_tiers` | `social.badge_definitions`, `social.badge_tiers` |
| `user_progress` | `user_progress` | `social.user_badge_progress`, `social.user_achieved_tiers` |

## Catalog mapping

Upstream's catalog becomes the node's; the node reads its catalog from these
tables, so every imported badge is served with no code change.

- Ids are upstream's, for badges and tiers alike, so `user_progress` and the
  earned tiers match them directly.
- Only published, unarchived badges are imported; the progress of a hidden
  badge is skipped (the load reports the count). Re-run with `--restart`
  after publishing it.
- A badge whose single tier sits in upstream's `root` slot is a single badge
  (`is_tier = false`, no tier rows, target 1). Every other badge keeps its
  tiers, `ordinal = position - 1`, `criteria_steps = steps`.
- `assets` follows upstream's layout:
  `<badge>[/<tier slot>]/2d/normal.png` and `3d/{normal,hrm,basecolor}.png`
  under `https://badges.decentraland.org/assets`, which the node rewrites to
  its public asset base. The node embeds art for its seed badges only: mirror
  upstream's art (the `CDN_HOST` bucket) into `BADGES_ASSETS_DIR` before
  cutover, or the other badges' images answer 404.
- Upstream's content replaces the seed's for the same ids (`emotionista`,
  `open_for_business`), tiers included: the seed's other tiers of an imported
  badge are deleted.
- The seed's placeholders for badges upstream names differently
  (`decentraland_citizen` = `dclcitizen`, `walkabout` =
  `walkabout_wanderer`) are deleted once upstream's are in, unless a player
  holds them on the node (reported).

## Columns

- `steps_done`: tiered badges take `progress.steps` (clamped to `integer`);
  single badges take 1 when completed, else 0, as the node's grant writes
  (their target is 1; upstream counts sub-goals, e.g. `open_for_business` 2).
- `completed_at`: upstream `completed_at` (all tiers done, or the single badge).
- `last_completed_tier_id`: the highest-ordinal imported tier.
- `user_achieved_tiers.completed_at`: the tier's own `completed_at`.
- `granted_by`/`granted_at`: NULL (event-consumer writes). Rows the console
  granted on the node keep their provenance.

Merges keep the furthest state: `GREATEST(steps_done)`, earliest
`completed_at`, highest-ordinal last tier, earliest tier `completed_at`.

## Not migrated

- `presence_badges`, `presence_visits`, `presence_applied_events`,
  `presence_pending_leaves`, `presence_absence_facts`: in-flight visit tracking
  and event dedupe of the upstream presence consumer; the node has none.
- `pgmigrations`.

## Left to the node

- `badge_admin_audit`: upstream keeps no grant log.

## Watermark

`updated_at`, moved by every progress write. Progress is small (one row per
wallet and badge): import it in full, then `--since last` until cutover.
