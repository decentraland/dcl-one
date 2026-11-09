# badges → social

Upstream: `processor/src/migrations` at the pinned revision: the badge
catalog (`badge_definitions`, `badge_tiers`, 34 badges and 84 tiers since
migration 1791331200000: 24 single badges with one `root` tier each, 10
tiered badges with 6 tiers each), the presence configuration
(`presence_badges`, migration 1792281600000) and `user_progress` (one row per
wallet and badge: `progress` document, `achieved_tiers`
`[{tier_id, completed_at}]`, epoch-ms `completed_at` and `updated_at`).

| Step | Upstream | Target |
| --- | --- | --- |
| `catalog` | `badge_definitions`, `badge_tiers` | `social.badge_definitions`, `social.badge_tiers` |
| `presence_badge` | `presence_badges` | `social.presence_badges` |
| `user_progress` | `user_progress` | `social.user_badge_progress`, `social.user_achieved_tiers` |

Every step needs the badge catalog schema: `badge_definitions.criteria_steps`,
`published_at`, `archived_at` and `created_at`,
`user_badge_progress.progress`, and `social.presence_badges`. On a schema
without them a step stops on its first non-empty batch (naming what is
missing) instead of importing without them; an empty batch is a no-op.

## Catalog mapping

Upstream's catalog becomes the node's; the node reads its catalog from these
tables, so every imported badge is served with no code change.

- Ids are upstream's, for badges and tiers alike, so `user_progress` and the
  earned tiers match them directly.
- Every badge is imported, published or not, archived or not, with
  `published_at`, `archived_at` and `created_at` (UTC). Hiding unpublished
  and archived badges from players who have not earned them is the node's
  read rule, as upstream's; their earners keep them.
- A badge whose single tier sits in upstream's `root` slot is a single badge
  (`is_tier = false`, no tier rows): the root tier's `steps` becomes
  `criteria_steps` (e.g. `open_for_business` 2), so the node serves
  upstream's target. Tiered badges keep their tiers (`ordinal = position - 1`,
  `criteria_steps = steps`) and have `criteria_steps` NULL.
- `assets` are upstream's URLs, built as upstream builds them
  (`common/src/utils/legacy-assets.ts`) under its production `CDN_HOST`
  `https://assets-cdn.decentraland.org`: a single badge's at
  `<badge>/2d/normal.png` and `<badge>/3d/{normal,hrm,basecolor}.png`, each
  tier's at `<badge>/<tier slot>/...`. A tiered badge has no art of its own
  (upstream's bucket holds none): its `assets` is `{}`, and the art served
  for it is its current tier's, or its first tier's when none is earned.
  `migrate.py blobs badges` mirrors every texture these URLs name into the
  node's `BADGES_ASSETS_DIR`.
- Upstream's content replaces the seed's for the same ids (`emotionista`,
  `open_for_business`), tiers included: the seed's other tiers of an imported
  badge are deleted, and so is a tier row whose position moved (it is
  re-inserted with upstream's, so no two tiers of a badge share an ordinal).
- The seed's placeholders for badges upstream names differently
  (`decentraland_citizen` = `dclcitizen`, `walkabout` =
  `walkabout_wanderer`) are deleted once upstream's are in, unless a player
  holds them on the node (reported).
- Not carried: `has_3d` (true for every upstream badge, and upstream rejects
  false), `updated_at`, the root tier's name and description (copies of the
  badge's).

## Presence configuration

`presence_badges` is catalog configuration (where, inside which UTC window,
visit or minimum stay), copied verbatim with upstream's column names and
types for the node's presence consumer.

## Columns

- `progress`: upstream's document verbatim: `steps` plus each evaluator's
  dedupe and resume state (`profiles_visited`, `scenes_titles_visited`,
  purchase tx hashes, `last_used_emote_timestamp`, the `open_for_business`
  sub-goal flags, presence `intervals`, ...).
- `steps_done`: `progress.steps` (clamped to `integer`) for every badge,
  single badges included (a partial `open_for_business` keeps its 1 of 2). A
  completed row without a numeric `steps` counts as its target.
- `completed_at`: upstream `completed_at` (all tiers done, or the single badge).
- `last_completed_tier_id`: the highest-ordinal imported tier.
- `user_achieved_tiers.completed_at`: the tier's own `completed_at`, else the
  badge's, else `updated_at`. Single badges have no tier rows.
- `granted_by`/`granted_at`: NULL (event-consumer writes). Rows the console
  granted on the node keep their provenance.

## Merges

Upstream writes lowercase addresses but does not enforce it, so rows of one
wallet under two spellings are merged, never dropped:

- `steps_done`: the most steps;
- `completed_at` and each tier's `completed_at`: the earliest;
- achieved tiers: the union; `last_completed_tier_id` the highest ordinal;
- `progress`: the document of the spelling with the most steps (then the
  latest `updated_at`, then the lowest address as written), each of its
  top-level arrays replaced by the distinct union of that key's arrays
  across the spellings (in jsonb order);
- `updated_at`: the latest.

The same rules merge into a row the node already holds, except `progress`,
where the source's document replaces the node's: no node writer keeps one,
so upstream's is the only state to resume from. `steps_done` never goes
down, so a node-side grant survives re-runs.

## Not migrated

- `presence_visits`, `presence_applied_events`, `presence_pending_leaves`,
  `presence_absence_facts`: in-flight visit tracking and event dedupe of the
  upstream presence consumer. Open visits at cutover are lost: cut over
  outside any presence window.
- `pgmigrations`.

## Left to the node

- `badge_admin_audit`: upstream keeps no grant log.

## Watermark and deletes

`user_progress` walks on `updated_at`, which every progress write moves:
import it in full, then `--since last` until cutover. `catalog` and
`presence_badge` have no watermark and re-scan in full on every run.
Deletes are not replicated: a `user_progress` row upstream deletes (its
dev-only reset), a badge or tier removed from the catalog, or a deleted
`presence_badges` row stays on the node. Upstream retires a badge by setting
`archived_at`, which the catalog step does carry.
