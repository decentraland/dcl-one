INSERT INTO social.user_mutes AS t (muter_address, muted_address, muted_at)
SELECT s.muter_address, s.muted_address, s.muted_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (muter_address, muted_address) DO UPDATE SET muted_at = EXCLUDED.muted_at;
