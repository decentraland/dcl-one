INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at)
SELECT place_id, banned_address, banned_by, banned_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (place_id, banned_address) DO UPDATE
SET banned_by = EXCLUDED.banned_by,
    banned_at = EXCLUDED.banned_at;
