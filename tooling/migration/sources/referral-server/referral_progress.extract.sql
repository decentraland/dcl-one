-- The earliest referral per invited wallet, as social-service-ea's
-- 1782864000000 dedupe keeps it. tier_granted wins over a stale status.
SELECT invited_user, referrer, status, created_at, updated_at
FROM (
    SELECT lower(invited_user) AS invited_user,
           lower(referrer) AS referrer,
           CASE WHEN tier_granted THEN 'tier_granted' ELSE status END AS status,
           to_timestamp(created_at / 1000.0) AS created_at,
           to_timestamp(updated_at / 1000.0) AS updated_at,
           row_number() OVER (PARTITION BY lower(invited_user) ORDER BY created_at, id) AS pick
    FROM referral_progress
    WHERE lower(invited_user) ~ '^0x[0-9a-f]{40}$'
      AND lower(referrer) ~ '^0x[0-9a-f]{40}$'
) p
WHERE pick = 1
  AND invited_user <> referrer
  AND status IN ('pending', 'signed_up', 'tier_granted')
