-- Keep the first referrer, completed progress and target reward execution state.
INSERT INTO social.referral_progress AS t
    (invited_user, referrer, status, invited_user_ip, signed_up_at, tier_granted,
     tier_granted_at, created_at, updated_at)
SELECT invited_user, referrer, status, invited_user_ip, signed_up_at AT TIME ZONE 'UTC',
       tier_granted, tier_granted_at AT TIME ZONE 'UTC',
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (invited_user) DO UPDATE SET
    status = CASE WHEN t.status = 'rejected_ip_match' THEN t.status
                  WHEN EXCLUDED.status = 'rejected_ip_match' THEN EXCLUDED.status
                  WHEN array_position(ARRAY['pending', 'signed_up', 'tier_granted'], EXCLUDED.status)
                     > array_position(ARRAY['pending', 'signed_up', 'tier_granted'], t.status)
                  THEN EXCLUDED.status ELSE t.status END,
    invited_user_ip = COALESCE(t.invited_user_ip, EXCLUDED.invited_user_ip),
    signed_up_at = COALESCE(t.signed_up_at, EXCLUDED.signed_up_at),
    tier_granted = t.tier_granted OR EXCLUDED.tier_granted,
    tier_granted_at = COALESCE(t.tier_granted_at, EXCLUDED.tier_granted_at),
    created_at = LEAST(t.created_at, EXCLUDED.created_at),
    updated_at = GREATEST(t.updated_at, EXCLUDED.updated_at)
WHERE t.referrer = EXCLUDED.referrer AND t.status <> 'rejected_ip_match';
