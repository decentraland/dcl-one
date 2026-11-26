BEGIN;
-- Genesis scene bans move from the scene entity to its place, the effective
-- base parcel (social.scene_base_parcel over the declared base that content
-- stores at entity_metadata -> 'v') that its admins use, so a redeploy
-- of the same place keeps them (schemas/40_social.sql). Two entities of one
-- place keep the newest ban of a wallet, also against a row the place has.
-- Every moved entity row is removed. A deployment counts while it is not
-- deleted (`deleter_deployment IS NULL`), as the room resolver reads it, also
-- an unpublished one. A row whose entity is not deployed, or has no parcel
-- that parses, stays. Stop old replicas before migration: old
-- readers and writers use the entity key. Run after `catalyst schema
-- migrate`. Rollback needs the matching backup.
WITH mapping AS (
    SELECT d.entity_id,
           social.scene_base_parcel(d.entity_pointers, d.entity_metadata #>> '{v,scene,base}') AS ban_key
      FROM content.deployments d
     WHERE d.entity_type = 'scene' AND d.deleter_deployment IS NULL
       AND d.entity_id IN (SELECT place_id FROM social.scene_bans)
), moved AS (
    INSERT INTO social.scene_bans AS t (place_id, banned_address, banned_by, banned_at)
    SELECT DISTINCT ON (m.ban_key, b.banned_address) m.ban_key, b.banned_address, b.banned_by, b.banned_at
      FROM social.scene_bans b JOIN mapping m ON m.entity_id = b.place_id
     WHERE m.ban_key IS NOT NULL
     ORDER BY m.ban_key, b.banned_address, b.banned_at DESC, b.banned_by
    ON CONFLICT (place_id, banned_address) DO UPDATE
        SET banned_by = EXCLUDED.banned_by, banned_at = EXCLUDED.banned_at
        WHERE EXCLUDED.banned_at > t.banned_at
)
DELETE FROM social.scene_bans b USING mapping m
 WHERE b.place_id = m.entity_id AND m.ban_key IS NOT NULL;

COMMIT;
