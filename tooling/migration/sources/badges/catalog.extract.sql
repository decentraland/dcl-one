-- One row per badge with its tiers in position order, published or not,
-- archived or not. A badge that is not tiered has exactly one tier, in the
-- `root` asset slot, carrying its steps. has_3d is not carried (always true).
-- presence and presence_steps are read as upstream's catalog reader does.
SELECT d.id,
       d.name,
       d.category,
       d.description,
       d.published_at,
       d.archived_at,
       d.created_at,
       EXISTS (SELECT 1 FROM presence_badges p WHERE p.badge_id = d.id) AS presence,
       (SELECT CASE WHEN p.mode = 'stay' THEN p.required_seconds ELSE 1 END
        FROM presence_badges p WHERE p.badge_id = d.id) AS presence_steps,
       COALESCE((SELECT jsonb_agg(jsonb_build_object(
                            'tier_id', t.tier_id, 'position', t.position, 'name', t.name,
                            'description', t.description, 'steps', t.steps, 'asset_slot', t.asset_slot)
                        ORDER BY t.position)
                 FROM badge_tiers t WHERE t.badge_id = d.id), '[]'::jsonb) AS tiers
FROM badge_definitions d
