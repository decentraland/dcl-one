-- The node's ban writes serialize per address on this lock.
DO $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtext(a)) FROM (SELECT DISTINCT banned_address AS a FROM stage ORDER BY 1) x;
END $$;

-- A lift recorded on the node is never undone by a re-read.
INSERT INTO social.user_bans AS t
    (id, banned_address, banned_by, reason, custom_message, banned_device_id,
     banned_at, expires_at, lifted_at, lifted_by, created_at)
SELECT id, banned_address, banned_by, reason, custom_message, banned_device_id,
       banned_at AT TIME ZONE 'UTC', expires_at AT TIME ZONE 'UTC',
       lifted_at AT TIME ZONE 'UTC', lifted_by, created_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (id) DO UPDATE
SET banned_address = EXCLUDED.banned_address,
    banned_by = EXCLUDED.banned_by,
    reason = EXCLUDED.reason,
    custom_message = EXCLUDED.custom_message,
    banned_device_id = EXCLUDED.banned_device_id,
    banned_at = EXCLUDED.banned_at,
    expires_at = EXCLUDED.expires_at,
    lifted_at = COALESCE(t.lifted_at, EXCLUDED.lifted_at),
    lifted_by = CASE WHEN t.lifted_at IS NOT NULL THEN t.lifted_by ELSE EXCLUDED.lifted_by END,
    created_at = EXCLUDED.created_at;
