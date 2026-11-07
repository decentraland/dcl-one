INSERT INTO social.referral_emails AS t (id, referrer, email, created_at, updated_at)
SELECT s.id, s.referrer, s.email, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (id) DO UPDATE SET
    referrer = EXCLUDED.referrer,
    email = EXCLUDED.email,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;
