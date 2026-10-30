-- World scene bans move from the scene entity id to the world and its base
-- parcel (comms::access::SceneBanKey, `<world>:<x>,<y>`), so a ban survives a
-- redeploy. A row keyed by a world scene still deployed here is rekeyed with
-- the base the room resolver derives (worlds::rooms footprint and
-- ParcelSet::effective_base: the stored parcels that parse as two i32s, then
-- the declared metadata.scene.base when it is one of them, else the lowest by
-- x then y); Genesis rows, rows of scenes already replaced and scenes without
-- a usable parcel stay as they are. When two scenes resolve to one key with a
-- ban of the same address, only the newest ban moves (DISTINCT ON), so the
-- update never collides on scene_bans_pkey. Queued scene removal intents keep
-- the entity id; the removal worker maps them (gatekeeper::removal).
-- Rollout: no overlap. Old-binary replicas still serving after this step
-- would write entity-id keys the new code never reads, so stop them before
-- `schema migrate` runs; the statement is idempotent if it must be re-run.
UPDATE social.scene_bans b
   SET place_id = k.ban_key
  FROM (
    SELECT DISTINCT ON (m.ban_key, s.banned_address) s.place_id, s.banned_address, m.ban_key
      FROM social.scene_bans s
      JOIN (
        SELECT ws.entity_id, ws.world_name || ':' || base.canonical AS ban_key
          FROM worlds.world_scenes ws
         CROSS JOIN LATERAL (
           SELECT f.canonical
             FROM (SELECT m[1]::bigint AS x, m[2]::bigint AS y,
                          (m[1]::bigint)::text || ',' || (m[2]::bigint)::text AS canonical
                     FROM unnest(ws.parcels) p,
                          regexp_match(p, '^\s*(-?[0-9]{1,10})\s*,\s*(-?[0-9]{1,10})\s*$') m
                    WHERE m IS NOT NULL) f
            WHERE f.x BETWEEN -2147483648 AND 2147483647
              AND f.y BETWEEN -2147483648 AND 2147483647
            ORDER BY f.canonical IS DISTINCT FROM (
                       SELECT (d[1]::bigint)::text || ',' || (d[2]::bigint)::text
                         FROM regexp_match(ws.entity #>> '{metadata,scene,base}',
                                           '^\s*(-?[0-9]{1,18})\s*,\s*(-?[0-9]{1,18})\s*$') d),
                     f.x, f.y
            LIMIT 1
         ) base
      ) m ON m.entity_id = s.place_id
     WHERE NOT EXISTS (SELECT 1 FROM social.scene_bans t
                        WHERE t.place_id = m.ban_key AND t.banned_address = s.banned_address)
     ORDER BY m.ban_key, s.banned_address, s.banned_at DESC
  ) k
 WHERE b.place_id = k.place_id AND b.banned_address = k.banned_address;
