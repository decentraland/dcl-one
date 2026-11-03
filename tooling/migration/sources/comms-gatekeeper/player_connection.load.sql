-- A newer node-side observation is kept; a null never erases a known value.
INSERT INTO social.player_connections (address, ip_address, device_id, created_at, updated_at)
SELECT address, ip_address, device_id, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (address) DO UPDATE
SET ip_address = COALESCE(EXCLUDED.ip_address, social.player_connections.ip_address),
    device_id = COALESCE(EXCLUDED.device_id, social.player_connections.device_id),
    created_at = LEAST(social.player_connections.created_at, EXCLUDED.created_at),
    updated_at = EXCLUDED.updated_at
WHERE EXCLUDED.updated_at >= social.player_connections.updated_at;
