-- First referrer wins, as on the node: a row already bound to another
-- referrer is left alone.
INSERT INTO social.referral_progress AS t (invited_user, referrer, status, created_at, updated_at)
SELECT s.invited_user, s.referrer, s.status, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (invited_user) DO UPDATE SET
    status = EXCLUDED.status,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE t.referrer = EXCLUDED.referrer;
