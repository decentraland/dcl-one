-- Preserve fresh published history during unrelated ingest; pending ownership changes fail closed.
CREATE OR REPLACE FUNCTION land_authz.scene_collaborators_current(
    configured_network text, configured_land text, configured_estate text,
    pointers text[], entity_timestamp timestamp without time zone
) RETURNS boolean LANGUAGE sql STABLE AS $$
  WITH ready AS (
    SELECT folded_block FROM land_authz.index_cursor
    WHERE network = configured_network
      AND land_registry = configured_land AND estate_registry = configured_estate
      AND full_replay_started AND history_timestamps_ready
      AND folded_block IS NOT NULL
      AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes'
                        AND timezone('UTC', now())
  ), parcels AS (
    SELECT p.pointer, t.token_id, t.owner, el.estate_token_id, et.owner AS estate_owner
    FROM unnest(pointers) AS p(pointer)
    LEFT JOIN land_authz.token_right t
      ON t.token_address = configured_land
     AND t.x = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 1)::integer END
     AND t.y = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 2)::integer END
     AND p.pointer = t.x::text || ',' || t.y::text
    LEFT JOIN land_authz.estate_land el ON el.land_token_id = t.token_id
    LEFT JOIN land_authz.token_right et
      ON et.token_address = configured_estate AND et.token_id = el.estate_token_id
  )
  SELECT COALESCE(
    entity_timestamp > timestamp '1970-01-01'
    AND entity_timestamp <= timezone('UTC', now())
    AND cardinality(pointers) > 0 AND EXISTS (SELECT 1 FROM ready)
    AND (SELECT bool_and(COALESCE(
      p.token_id IS NOT NULL AND p.owner IS NOT NULL
      AND p.owner <> '0x0000000000000000000000000000000000000000'
      AND ((p.owner = configured_estate) = (p.estate_token_id IS NOT NULL))
      AND lt.block_number <= (SELECT folded_block FROM ready)
      AND lt.block_time > 0
      AND lt.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')
      AND (membership.block_number IS NULL OR (
        membership.block_number <= (SELECT folded_block FROM ready)
        AND membership.block_time > 0
        AND membership.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
      AND (p.estate_token_id IS NULL OR (
        p.estate_owner IS NOT NULL
        AND p.estate_owner <> '0x0000000000000000000000000000000000000000'
        AND et.block_number <= (SELECT folded_block FROM ready)
        AND et.block_time > 0
        AND et.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
    , false)) FROM parcels p
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_land AND token_id = p.token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) lt ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.token_id
        AND kind IN ('add_land', 'remove_land')
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) membership ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.estate_token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) et ON true), false)
$$;
