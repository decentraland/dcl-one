SELECT lower(p.invited_user) AS invited_user,
       lower(p.referrer) AS referrer,
       CASE WHEN p.status = 'rejected_ip_match' THEN p.status WHEN p.tier_granted THEN 'tier_granted' ELSE p.status END AS status,
       p.invited_user_ip,
       to_timestamp(p.signed_up_at / 1000.0) AS signed_up_at,
       (p.tier_granted OR p.status = 'tier_granted') AS tier_granted,
       to_timestamp(p.tier_granted_at / 1000.0) AS tier_granted_at,
       to_timestamp(p.created_at / 1000.0) AS created_at,
       to_timestamp(p.updated_at / 1000.0) AS updated_at
FROM referral_progress p
WHERE lower(p.invited_user) ~ '^0x[0-9a-f]{40}$'
  AND lower(p.referrer) ~ '^0x[0-9a-f]{40}$'
  AND p.status IN ('pending', 'signed_up', 'tier_granted', 'rejected_ip_match')
  AND lower(p.invited_user) <> lower(p.referrer)
  AND NOT EXISTS (
      SELECT 1 FROM referral_progress d
      WHERE lower(d.invited_user) = lower(p.invited_user)
        AND (d.created_at, d.id) < (p.created_at, p.id))
