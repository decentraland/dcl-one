SELECT e.id,
       lower(e.referrer) AS referrer,
       e.email,
       to_timestamp(e.created_at / 1000.0) AS created_at,
       to_timestamp(e.updated_at / 1000.0) AS updated_at
FROM referral_emails e
WHERE char_length(e.email) <= 254
