-- Upstream timestamps are epoch milliseconds. invited_user_ip,
-- signed_up_at, tier_granted and tier_granted_at have no target column
-- (status carries the progress); the IP is personal data and stays behind.
SELECT lower(p.invited_user) AS invited_user,
       lower(p.referrer) AS referrer,
       p.status::text AS status,
       to_timestamp(p.created_at / 1000.0) AS created_at,
       to_timestamp(p.updated_at / 1000.0) AS updated_at
FROM referral_progress p
WHERE p.status IN ('pending', 'signed_up', 'tier_granted', 'rejected_ip_match')
  AND lower(p.invited_user) <> lower(p.referrer)
  AND NOT EXISTS (
      SELECT 1 FROM referral_progress d
      WHERE lower(d.invited_user) = lower(p.invited_user)
        AND (d.created_at, d.id) < (p.created_at, p.id))
