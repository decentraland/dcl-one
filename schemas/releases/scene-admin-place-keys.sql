BEGIN;
-- Scene admin grants move from the scene entity to its place, so a redeploy
-- of the same place keeps them (schemas/40_social.sql): a world scene's
-- active identity (explore.world_scene_place), a Genesis scene's effective
-- base parcel (social.scene_base_parcel over the declared base that content
-- stores at entity_metadata -> 'v'), both as the room resolver reads
-- them. An entity that is active in several places gives each place the
-- grant. A place keeps one row for a wallet: an active grant before a
-- revoked one, then the newest change, also against a row the place has.
-- Every moved entity row is removed. A row whose entity is not deployed, or
-- has no parcel that parses, stays. The script can run again: it moves only
-- rows that have an entity key, and never changes the key of a row that is
-- on a parcel or a world-scene identity. Stop old replicas before migration: old
-- readers and writers use the entity key. Run after `catalyst schema
-- migrate`. Rollback needs the matching backup.
WITH mapping AS (
    SELECT * FROM (
        SELECT i.entity_id, i.identity::text AS place
          FROM explore.world_scene_place i
         WHERE i.active
        UNION ALL
        SELECT d.entity_id,
               social.scene_base_parcel(d.entity_pointers, d.entity_metadata #>> '{v,scene,base}')
          FROM content.deployments d
         WHERE d.entity_type = 'scene' AND d.deleter_deployment IS NULL
    ) m
     WHERE place IS NOT NULL AND entity_id IN (SELECT place_id FROM social.scene_admins)
), moved AS (
    INSERT INTO social.scene_admins AS t (place_id, admin_address, added_by, added_at, active, removed_at)
    SELECT DISTINCT ON (m.place, a.admin_address)
           m.place, a.admin_address, a.added_by, a.added_at, a.active, a.removed_at
      FROM social.scene_admins a JOIN mapping m ON m.entity_id = a.place_id
     ORDER BY m.place, a.admin_address, a.active DESC, COALESCE(a.removed_at, a.added_at) DESC, a.place_id
    ON CONFLICT (place_id, admin_address) DO UPDATE
        SET added_by = EXCLUDED.added_by, added_at = EXCLUDED.added_at,
            active = EXCLUDED.active, removed_at = EXCLUDED.removed_at
        WHERE COALESCE(t.removed_at, t.added_at) <= COALESCE(EXCLUDED.removed_at, EXCLUDED.added_at)
)
DELETE FROM social.scene_admins a USING mapping m WHERE a.place_id = m.entity_id;

COMMIT;
