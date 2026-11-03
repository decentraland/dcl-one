SELECT address, ip_address, device_id, created_at, updated_at
FROM (
    SELECT lower(address) AS address,
           ip_address,
           device_id,
           to_timestamp(created_at / 1000.0) AS created_at,
           to_timestamp(updated_at / 1000.0) AS updated_at,
           row_number() OVER (PARTITION BY lower(address) ORDER BY updated_at DESC) AS pick
    FROM player_connection_info
    WHERE lower(address) ~ '^0x[0-9a-f]{40}$'
) c
WHERE pick = 1
