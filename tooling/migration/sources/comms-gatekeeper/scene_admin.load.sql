-- Resolve the upstream Places UUID through the imported active catalogue.
-- A missing or ambiguous identity must stop cutover, never silently drop a ban/grant.
CREATE TEMP TABLE migration_scene_admin_identity ON COMMIT DROP AS
WITH candidates AS (
    SELECT DISTINCT s.place_id AS source_id, d.entity_id AS target_id
    FROM stage s
    JOIN explore.place p ON p.id = s.place_id AND NOT p.world
    JOIN content.deployments d ON d.entity_id = p.raw->>'deployment_id'
        AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL
    WHERE EXISTS (SELECT 1 FROM content.active_pointers a WHERE a.entity_id = d.entity_id)
    UNION
    SELECT DISTINCT s.place_id, ws.entity_id
    FROM stage s
    JOIN explore.world_scene_place i ON i.identity::text = lower(s.place_id) AND i.active
    JOIN worlds.world_scenes ws ON ws.world_name = i.world_name AND ws.entity_id = i.entity_id
)
SELECT source_id, min(target_id) AS target_id, count(*) AS candidates
FROM candidates GROUP BY source_id;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM stage s LEFT JOIN migration_scene_admin_identity m ON m.source_id=s.place_id
               WHERE m.target_id IS NULL OR m.candidates <> 1) THEN
        RAISE EXCEPTION 'unresolved or ambiguous comms scene_admin Places identity; import the active catalogue and repair source identities before cutover';
    END IF;
END $$;

-- Upstream records no revocation time; a revoked grant gets its grant time.
-- A node grant or revocation newer than the upstream grant wins.
INSERT INTO social.scene_admins AS t (place_id, admin_address, added_by, added_at, active, removed_at)
SELECT DISTINCT ON (m.target_id, admin_address) m.target_id, admin_address, added_by, added_at AT TIME ZONE 'UTC', active,
       CASE WHEN active THEN NULL ELSE added_at AT TIME ZONE 'UTC' END
FROM stage s JOIN migration_scene_admin_identity m ON m.source_id=s.place_id
ORDER BY m.target_id, admin_address, active DESC, added_at DESC, s.place_id, added_by
ON CONFLICT (place_id, admin_address) DO UPDATE
SET added_by = EXCLUDED.added_by,
    added_at = EXCLUDED.added_at,
    active = EXCLUDED.active,
    removed_at = EXCLUDED.removed_at
WHERE COALESCE(t.removed_at, t.added_at) <= EXCLUDED.added_at;

DROP TABLE migration_scene_admin_identity;
