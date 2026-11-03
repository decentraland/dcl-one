INSERT INTO social.user_bans
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
    lifted_at = EXCLUDED.lifted_at,
    lifted_by = EXCLUDED.lifted_by,
    created_at = EXCLUDED.created_at;
