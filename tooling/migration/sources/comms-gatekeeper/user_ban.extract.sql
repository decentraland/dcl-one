-- timestamp without time zone columns holding UTC.
SELECT id,
       lower(banned_address) AS banned_address,
       CASE WHEN banned_by ~* '^0x[0-9a-f]{40}$' THEN lower(banned_by) ELSE banned_by END AS banned_by,
       reason,
       custom_message,
       banned_device_id,
       banned_at AT TIME ZONE 'UTC' AS banned_at,
       expires_at AT TIME ZONE 'UTC' AS expires_at,
       lifted_at AT TIME ZONE 'UTC' AS lifted_at,
       CASE WHEN lifted_at IS NULL THEN NULL
            WHEN lifted_by ~* '^0x[0-9a-f]{40}$' THEN lower(lifted_by)
            ELSE COALESCE(lifted_by, '') END AS lifted_by,
       created_at AT TIME ZONE 'UTC' AS created_at
FROM user_bans
WHERE lower(banned_address) ~ '^0x[0-9a-f]{40}$'
