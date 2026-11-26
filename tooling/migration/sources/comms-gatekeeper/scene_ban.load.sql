-- Resolve the upstream Places UUID through the imported active catalogue to
-- the key the node bans on: a Genesis scene's effective base parcel
-- (social.scene_base_parcel over the declared base that content stores at
-- entity_metadata -> 'v'), a world scene's `world_name:effective_base`.
-- A missing or ambiguous identity must stop cutover, never silently drop a ban/grant.
CREATE TEMP TABLE migration_scene_ban_identity ON COMMIT DROP AS
WITH candidates AS (
    SELECT DISTINCT s.place_id AS source_id, base.key AS target_id
    FROM stage s
    JOIN explore.place p ON p.id = s.place_id AND NOT p.world
    JOIN content.deployments d ON d.entity_id = p.raw->>'deployment_id'
        AND d.entity_type = 'scene' AND d.deleter_deployment IS NULL
    CROSS JOIN LATERAL (SELECT social.scene_base_parcel(d.entity_pointers, d.entity_metadata #>> '{v,scene,base}') AS key) base
    WHERE base.key IS NOT NULL AND EXISTS (SELECT 1 FROM content.active_pointers a WHERE a.entity_id = d.entity_id)
    UNION
    SELECT DISTINCT s.place_id, ws.world_name || ':' || base.canonical
    FROM stage s
    JOIN explore.world_scene_place i ON i.identity::text = lower(s.place_id) AND i.active
    JOIN worlds.world_scenes ws ON ws.world_name = i.world_name AND ws.entity_id = i.entity_id
    CROSS JOIN LATERAL (SELECT social.scene_base_parcel(ws.parcels, ws.entity #>> '{metadata,scene,base}') AS canonical) base
    WHERE base.canonical IS NOT NULL
)
SELECT source_id, min(target_id) AS target_id, count(*) AS candidates
FROM candidates GROUP BY source_id;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM stage s LEFT JOIN migration_scene_ban_identity m ON m.source_id=s.place_id
               WHERE m.target_id IS NULL OR m.candidates <> 1) THEN
        RAISE EXCEPTION 'unresolved or ambiguous comms scene_ban Places identity; import the active catalogue and repair source identities before cutover';
    END IF;
END $$;

INSERT INTO social.scene_bans (place_id, banned_address, banned_by, banned_at)
SELECT DISTINCT ON (m.target_id, banned_address) m.target_id, banned_address, banned_by, banned_at AT TIME ZONE 'UTC'
FROM stage s JOIN migration_scene_ban_identity m ON m.source_id=s.place_id
ORDER BY m.target_id, banned_address, banned_at DESC, s.place_id, banned_by
ON CONFLICT (place_id, banned_address) DO UPDATE
SET banned_by = EXCLUDED.banned_by,
    banned_at = EXCLUDED.banned_at;

DROP TABLE migration_scene_ban_identity;
