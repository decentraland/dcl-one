SELECT lower(v.referrer) AS referrer,
       v.invites_accepted_viewed::bigint AS invites_accepted_viewed,
       to_timestamp(v.updated_at / 1000.0) AS updated_at
FROM referral_progress_viewed v
WHERE NOT EXISTS (
    SELECT 1 FROM referral_progress_viewed d
    WHERE lower(d.referrer) = lower(v.referrer)
      AND (d.updated_at, d.referrer) > (v.updated_at, v.referrer))
