-- Upstream's id is the place id (catalogue uuid text or a world name).
SELECT p.id AS place_id,
       p.community_id,
       lower(p.added_by) AS added_by,
       p.added_at AT TIME ZONE 'UTC' AS added_at
FROM community_places p
