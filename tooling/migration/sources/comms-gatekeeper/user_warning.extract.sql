SELECT id,
       lower(warned_address) AS warned_address,
       CASE WHEN warned_by ~* '^0x[0-9a-f]{40}$' THEN lower(warned_by) ELSE warned_by END AS warned_by,
       reason,
       warned_at AT TIME ZONE 'UTC' AS warned_at
FROM user_warnings
WHERE lower(warned_address) ~ '^0x[0-9a-f]{40}$'
