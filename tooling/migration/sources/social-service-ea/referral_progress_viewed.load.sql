INSERT INTO social.referral_progress_viewed AS t (referrer, invites_accepted_viewed, updated_at)
SELECT s.referrer, s.invites_accepted_viewed, s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (referrer) DO UPDATE SET
    invites_accepted_viewed = EXCLUDED.invites_accepted_viewed,
    updated_at = EXCLUDED.updated_at
WHERE t.updated_at <= EXCLUDED.updated_at;
