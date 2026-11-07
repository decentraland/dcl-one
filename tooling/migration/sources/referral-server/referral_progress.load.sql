-- First referrer wins; the status only moves forward and never leaves
-- rejected_ip_match.
INSERT INTO social.referral_progress (invited_user, referrer, status, created_at, updated_at)
SELECT invited_user, referrer, status, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (invited_user) DO UPDATE
SET status = EXCLUDED.status,
    created_at = LEAST(social.referral_progress.created_at, EXCLUDED.created_at),
    updated_at = GREATEST(social.referral_progress.updated_at, EXCLUDED.updated_at)
WHERE social.referral_progress.referrer = EXCLUDED.referrer
  AND array_position(ARRAY['pending', 'signed_up', 'tier_granted'], EXCLUDED.status)
      > array_position(ARRAY['pending', 'signed_up', 'tier_granted'], social.referral_progress.status);
