INSERT INTO social.blocks AS t (blocker_address, blocked_address, blocked_at)
SELECT s.blocker_address, s.blocked_address, s.blocked_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (blocker_address, blocked_address) DO UPDATE SET blocked_at = EXCLUDED.blocked_at;
