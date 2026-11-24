-- Upstream's id is the place id (catalogue uuid text or a world name), lowercased as the node
-- stores it; of case variants in one community the lowercase spelling wins, then the earliest.
SELECT lower(p.id) AS place_id,
       p.community_id,
       lower(p.added_by) AS added_by,
       p.added_at AT TIME ZONE 'UTC' AS added_at
FROM community_places p
WHERE NOT EXISTS (
    SELECT 1 FROM community_places d
    WHERE d.community_id = p.community_id
      AND lower(d.id) = lower(p.id)
      AND (d.id <> lower(d.id), d.added_at, d.id) < (p.id <> lower(p.id), p.added_at, p.id))
