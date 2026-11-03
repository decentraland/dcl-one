INSERT INTO social.user_warnings (id, warned_address, warned_by, reason, warned_at)
SELECT id, warned_address, warned_by, reason, warned_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (id) DO UPDATE
SET warned_address = EXCLUDED.warned_address,
    warned_by = EXCLUDED.warned_by,
    reason = EXCLUDED.reason,
    warned_at = EXCLUDED.warned_at;
