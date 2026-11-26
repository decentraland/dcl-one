-- A local world catalogue row pruned because the worlds mirror took the
-- world over no longer queues a place removal while a scene place of it is
-- active.
CREATE OR REPLACE FUNCTION explore.capture_place_removal() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    place_key text;
    snapshot record;
    was_removed boolean;
    now_removed boolean;
    effective_reason text;
    event_admin_keys text[];
    first_seen boolean;
BEGIN
    place_key := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    PERFORM pg_advisory_xact_lock(hashtext('explore.place-removal:' || place_key));
    SELECT p.* INTO snapshot FROM (
        SELECT id, creator_address, world, world_name, base_position, raw, removal_admin_keys, disabled, 0 AS priority FROM explore.place_world_local WHERE id = place_key
        UNION ALL
        SELECT id, creator_address, world, world_name, base_position, raw, removal_admin_keys, disabled, 1 AS priority FROM explore.place WHERE id = place_key
    ) p ORDER BY priority LIMIT 1;
    IF NOT FOUND THEN
        IF TG_OP <> 'DELETE' THEN RETURN NULL; END IF;
        snapshot := OLD;
        effective_reason := COALESCE((SELECT reason FROM explore.place_moderation WHERE place_id = place_key AND disabled IS NOT NULL), OLD.raw->>'disabled_reason');
        -- A local world row pruned because the worlds mirror took the world
        -- over is not a removal while a scene place of it is active: its place
        -- ids live on. A blocked world, or one with no active scene, is.
        now_removed := effective_reason IS DISTINCT FROM 'opt_out' AND NOT (
            TG_TABLE_NAME = 'place_world_local' AND OLD.world
            AND EXISTS (SELECT 1 FROM worlds.worlds
                        WHERE name = lower(COALESCE(OLD.world_name, OLD.id))
                          AND mirror_source IS NOT NULL AND blocked_since IS NULL)
            AND EXISTS (SELECT 1 FROM explore.world_scene_place
                        WHERE world_name = lower(COALESCE(OLD.world_name, OLD.id)) AND active));
    ELSE
        effective_reason := COALESCE((SELECT reason FROM explore.place_moderation WHERE place_id = place_key AND disabled IS NOT NULL), snapshot.raw->>'disabled_reason');
        now_removed := snapshot.disabled AND effective_reason IS DISTINCT FROM 'opt_out';
    END IF;
    INSERT INTO explore.place_removal_state (place_id, removed) VALUES (place_key, false) ON CONFLICT DO NOTHING;
    -- A place first seen already disabled (an import or mirror catch-up adding a
    -- row neither leg held) is historical, like the seed: cleaned, not notified.
    -- A state row alone is no evidence: upgraded catalogues seed only disabled rows.
    first_seen := FOUND AND TG_OP = 'INSERT' AND NOT EXISTS (
        SELECT 1 FROM explore.place WHERE id = place_key AND TG_TABLE_NAME <> 'place'
        UNION ALL
        SELECT 1 FROM explore.place_world_local WHERE id = place_key AND TG_TABLE_NAME <> 'place_world_local');
    SELECT removed INTO was_removed FROM explore.place_removal_state WHERE place_id = place_key FOR UPDATE;
    IF now_removed AND NOT was_removed THEN
        event_admin_keys := ARRAY(SELECT DISTINCT unnest(snapshot.removal_admin_keys || CASE WHEN snapshot.world THEN ARRAY(SELECT identity::text FROM explore.world_scene_place WHERE world_name=lower(snapshot.world_name) AND active) ELSE '{}'::text[] END) ORDER BY 1);
        -- Freeze Genesis admins with the event: a re-grant before maintenance
        -- can replace added_at and erase the earlier grant's time evidence. World
        -- admins need their granter's authority checked, so their exact scene
        -- places (admin_keys) and each active admin with its original granter
        -- (admin_grants) are frozen; maintenance checks those granters later.
        -- Read through MVCC without taking admin locks after catalogue locks.
        INSERT INTO explore.place_removal_pending (place_id, owner_address, world, world_name, base_position, positions, admin_keys, admin_grants, recipients, notify)
        VALUES (place_key, CASE WHEN snapshot.world THEN COALESCE(snapshot.raw->>'owner', snapshot.creator_address) ELSE snapshot.raw->>'owner' END,
            snapshot.world, snapshot.world_name, snapshot.base_position, COALESCE(snapshot.raw->'positions', jsonb_build_array(snapshot.base_position)), event_admin_keys,
            COALESCE((SELECT jsonb_agg(DISTINCT jsonb_build_array(lower(btrim(admin_address)), lower(btrim(added_by))))
                      FROM social.scene_admins WHERE active AND snapshot.world AND place_id = ANY(event_admin_keys)), '[]'::jsonb),
            ARRAY(SELECT DISTINCT lower(btrim(admin_address)) FROM social.scene_admins
                  WHERE active AND lower(btrim(admin_address)) ~ '^0x[0-9a-f]{40}$'
                    AND NOT snapshot.world AND place_id=snapshot.base_position ORDER BY 1),
            NOT first_seen);
    END IF;
    UPDATE explore.place_removal_state SET removed = now_removed WHERE place_id = place_key AND removed IS DISTINCT FROM now_removed;
    IF NOT now_removed AND NOT EXISTS (SELECT 1 FROM explore.place WHERE id = place_key) AND NOT EXISTS (SELECT 1 FROM explore.place_world_local WHERE id = place_key) AND NOT EXISTS (SELECT 1 FROM explore.place_removal_pending WHERE place_id = place_key) THEN
        DELETE FROM explore.place_removal_state WHERE place_id = place_key;
    END IF;
    RETURN NULL;
END;
$$;
