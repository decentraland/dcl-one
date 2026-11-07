-- Upstream records no revocation time; a revoked grant gets its grant time.
INSERT INTO social.scene_admins (place_id, admin_address, added_by, added_at, active, removed_at)
SELECT place_id, admin_address, added_by, added_at AT TIME ZONE 'UTC', active,
       CASE WHEN active THEN NULL ELSE added_at AT TIME ZONE 'UTC' END
FROM stage
ON CONFLICT (place_id, admin_address) DO UPDATE
SET added_by = EXCLUDED.added_by,
    added_at = EXCLUDED.added_at,
    active = EXCLUDED.active,
    removed_at = EXCLUDED.removed_at;
