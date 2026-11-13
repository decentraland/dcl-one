-- Native player-storage command boundary shared by public handlers and isolated replay.
-- The caller supplies its already-resolved namespace and validated JSON serialization.
CREATE OR REPLACE FUNCTION worlds.apply_player_storage_command_v1(
    p_op text, p_realm text, p_namespace text, p_shared boolean,
    p_player text, p_key text, p_value text, p_size bigint,
    p_max_value bigint, p_max_total bigint,
    p_created timestamp, p_updated timestamp
) RETURNS TABLE(existing bigint, total bigint, inserted boolean)
LANGUAGE plpgsql SET search_path=pg_catalog AS $$
DECLARE lock_key text;
BEGIN
    IF p_op IS NULL OR p_op NOT IN ('set','delete','clear_player','clear_scene') OR p_realm IS NULL
       OR p_namespace IS NULL OR p_shared IS NULL THEN
        RAISE EXCEPTION 'invalid player storage command';
    END IF;
    IF p_op <> 'clear_scene' AND (p_player IS NULL OR p_player !~ '^0x[0-9a-f]{40}$') THEN
        RAISE EXCEPTION 'invalid player storage actor';
    END IF;
    IF p_op IN ('set','delete') AND (p_key IS NULL OR length(p_key) NOT BETWEEN 1 AND 255) THEN
        RAISE EXCEPTION 'invalid player storage key';
    END IF;
    existing:=0; total:=0; inserted:=true;
    IF p_op='set' THEN
        IF p_value IS NULL OR p_size IS NULL OR p_size<>octet_length(p_value) OR p_size<0
          OR p_max_value IS NULL OR p_max_total IS NULL OR p_max_value<0 OR p_max_total<0
          OR p_created IS NULL OR p_updated IS NULL THEN RAISE EXCEPTION 'invalid player storage value'; END IF;
        lock_key:='player-storage:'||p_realm||CASE WHEN p_shared THEN ':'||p_namespace ELSE '' END||':'||p_player;
        PERFORM pg_advisory_xact_lock(hashtextextended(lock_key,0));
        SELECT COALESCE(MAX(s.value_size) FILTER(WHERE s.scene_namespace=p_namespace AND s.key=p_key),0)::bigint,
               COALESCE(SUM(s.value_size),0)::bigint INTO existing,total
          FROM worlds.player_storage s WHERE s.realm=p_realm AND s.player_address=p_player
          AND (CASE WHEN p_shared THEN s.scene_namespace=p_namespace ELSE
            NOT EXISTS(SELECT 1 FROM explore.world_scene_place i WHERE i.world_name=p_realm AND NOT i.active AND i.identity::text=s.scene_namespace)
            AND (s.scene_namespace<>'world' OR NOT EXISTS(SELECT 1 FROM explore.world_scene_place i WHERE i.world_name=p_realm AND i.active)) END);
        inserted:=p_size<=p_max_value AND total-existing+p_size<=p_max_total;
        IF inserted THEN
          INSERT INTO worlds.player_storage(realm,scene_namespace,player_address,key,value,value_size,created_at,updated_at)
            VALUES(p_realm,p_namespace,p_player,p_key,p_value::jsonb,p_size::integer,p_created,p_updated)
            ON CONFLICT(realm,scene_namespace,player_address,key) DO UPDATE
              SET value=EXCLUDED.value,value_size=EXCLUDED.value_size,updated_at=EXCLUDED.updated_at;
        END IF;
    ELSE
        DELETE FROM worlds.player_storage s WHERE s.realm=p_realm AND s.scene_namespace=p_namespace
          AND (p_op='clear_scene' OR s.player_address=p_player)
          AND (p_op<>'delete' OR s.key=p_key);
    END IF;
    RETURN NEXT;
END $$;
