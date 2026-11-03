-- Operator decisions first, as overlays the places mirror and derive re-apply
-- (places::moderation, places::ranking). Rows this migration wrote carry
-- set_by = 'migration:places'; a decision made on the node (any other set_by)
-- is never overwritten, and a migration overlay whose upstream decision is gone
-- is removed.
DELETE FROM explore.place_moderation m
USING stage s
WHERE m.place_id = s.id AND m.set_by = 'migration:places'
  AND NOT (s.mod_disabled OR s.highlighted OR s.mod_rating IS NOT NULL);

INSERT INTO explore.place_moderation AS m
    (place_id, disabled, highlighted, content_rating, reason, set_by, updated_at)
SELECT id,
       CASE WHEN mod_disabled THEN true END,
       CASE WHEN highlighted THEN true END,
       mod_rating,
       CASE WHEN mod_disabled THEN 'moderation' END,
       'migration:places',
       timezone('UTC', now())
FROM stage
WHERE mod_disabled OR highlighted OR mod_rating IS NOT NULL
ON CONFLICT (place_id) DO UPDATE SET
    disabled = EXCLUDED.disabled,
    highlighted = EXCLUDED.highlighted,
    content_rating = EXCLUDED.content_rating,
    reason = EXCLUDED.reason,
    updated_at = EXCLUDED.updated_at
WHERE m.set_by = 'migration:places'
  AND (m.disabled, m.highlighted, m.content_rating, m.reason)
      IS DISTINCT FROM (EXCLUDED.disabled, EXCLUDED.highlighted, EXCLUDED.content_rating, EXCLUDED.reason);

DELETE FROM explore.place_ranking r
USING stage s
WHERE r.place_id = s.id AND r.set_by = 'migration:places' AND s.ranking_overlay IS NULL;

INSERT INTO explore.place_ranking AS r (place_id, overlay, set_by, updated_at)
SELECT id, ranking_overlay, 'migration:places', timezone('UTC', now())
FROM stage
WHERE ranking_overlay IS NOT NULL
ON CONFLICT (place_id) DO UPDATE SET
    overlay = EXCLUDED.overlay,
    updated_at = EXCLUDED.updated_at
WHERE r.set_by = 'migration:places' AND r.overlay IS DISTINCT FROM EXCLUDED.overlay;

-- The catalogue row, local authority: counters start at 0 because the facts
-- (user_favorites, user_likes) are imported and counted in
-- place_interaction_counts. A world or id this node already publishes in the
-- local leg is left to it, and a row derived from this node's content
-- (raw.source = 'content') is never overwritten, as the mirror does.
INSERT INTO explore.place AS p
    (id, base_position, title, description, creator_address, content_rating,
     categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
     raw, fetched_at, origin)
SELECT s.id, s.base_position, s.title, s.description, s.creator_address,
       COALESCE(m.content_rating, s.content_rating),
       COALESCE(s.categories, '{}'), 0, 0, 0,
       s.deployed_at AT TIME ZONE 'UTC',
       COALESCE(m.disabled, s.disabled), COALESCE(m.highlighted, s.highlighted),
       s.raw || COALESCE(r.overlay, '{}'::jsonb),
       timezone('UTC', now()), 'local'
FROM stage s
LEFT JOIN explore.place_moderation m ON m.place_id = s.id
LEFT JOIN explore.place_ranking r ON r.place_id = s.id
WHERE NOT EXISTS (
    SELECT 1 FROM explore.place_world_local l
    WHERE l.id = s.id
       OR (s.world_name IS NOT NULL AND l.world IS TRUE AND lower(l.world_name) = lower(s.world_name)))
ON CONFLICT (id) DO UPDATE SET
    base_position   = EXCLUDED.base_position,
    title           = EXCLUDED.title,
    description     = EXCLUDED.description,
    creator_address = EXCLUDED.creator_address,
    content_rating  = EXCLUDED.content_rating,
    categories      = EXCLUDED.categories,
    likes           = 0,
    dislikes        = 0,
    favorites       = 0,
    deployed_at     = EXCLUDED.deployed_at,
    disabled        = EXCLUDED.disabled,
    highlighted     = EXCLUDED.highlighted,
    raw             = EXCLUDED.raw,
    fetched_at      = EXCLUDED.fetched_at,
    origin          = 'local'
WHERE p.raw->>'source' IS DISTINCT FROM 'content'
  AND (p.base_position, p.title, p.description, p.creator_address, p.content_rating,
       p.categories, p.likes, p.dislikes, p.favorites, p.deployed_at, p.disabled,
       p.highlighted, p.raw, p.origin)
      IS DISTINCT FROM
      (EXCLUDED.base_position, EXCLUDED.title, EXCLUDED.description, EXCLUDED.creator_address,
       EXCLUDED.content_rating, EXCLUDED.categories, 0, 0, 0, EXCLUDED.deployed_at,
       EXCLUDED.disabled, EXCLUDED.highlighted, EXCLUDED.raw, 'local');
