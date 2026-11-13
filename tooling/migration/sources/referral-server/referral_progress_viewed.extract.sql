SELECT referrer, invites_accepted_viewed, updated_at
FROM (
    SELECT lower(referrer) AS referrer,
           invites_accepted_viewed::bigint AS invites_accepted_viewed,
           to_timestamp(updated_at / 1000.0) AS updated_at,
           row_number() OVER (PARTITION BY lower(referrer)
                              ORDER BY invites_accepted_viewed DESC, updated_at DESC) AS pick
    FROM referral_progress_viewed
    WHERE lower(referrer) ~ '^0x[0-9a-f]{40}$'
) v
WHERE pick = 1
