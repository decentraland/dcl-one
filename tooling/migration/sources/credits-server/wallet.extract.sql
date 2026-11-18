-- One row per wallet in the program. opted_out_at ends participation.
-- Upstream flags are not the node's operator block: they go to wallet_flag.
SELECT lower(u.address) AS address,
       (u.opted_out_at IS NULL) AS has_started_program,
       u.created_at AT TIME ZONE 'UTC' AS created_at,
       u.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM users u
