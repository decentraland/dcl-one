-- One row per badge with its tiers in position order, published or not,
-- archived or not. A badge that is not tiered has exactly one tier, in the
-- `root` asset slot, carrying its steps. has_3d is not carried (always true).
SELECT d.id,
       d.name,
       d.category,
       d.description,
       d.published_at,
       d.archived_at,
       d.created_at,
       COALESCE((SELECT jsonb_agg(jsonb_build_object(
                            'tier_id', t.tier_id, 'position', t.position, 'name', t.name,
                            'description', t.description, 'steps', t.steps, 'asset_slot', t.asset_slot)
                        ORDER BY t.position)
                 FROM badge_tiers t WHERE t.badge_id = d.id), '[]'::jsonb) AS tiers
FROM badge_definitions d
