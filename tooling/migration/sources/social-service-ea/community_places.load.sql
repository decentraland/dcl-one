INSERT INTO social.community_places AS t (place_id, community_id, added_by, added_at)
SELECT s.place_id, s.community_id, s.added_by, s.added_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
ON CONFLICT (place_id, community_id) DO UPDATE SET
    added_by = EXCLUDED.added_by,
    added_at = EXCLUDED.added_at;
