-- The count a referrer last saw only grows.
INSERT INTO social.referral_progress_viewed (referrer, invites_accepted_viewed, updated_at)
SELECT referrer, GREATEST(invites_accepted_viewed, 0), updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (referrer) DO UPDATE
SET invites_accepted_viewed = EXCLUDED.invites_accepted_viewed,
    updated_at = GREATEST(social.referral_progress_viewed.updated_at, EXCLUDED.updated_at)
WHERE EXCLUDED.invites_accepted_viewed > social.referral_progress_viewed.invites_accepted_viewed;
