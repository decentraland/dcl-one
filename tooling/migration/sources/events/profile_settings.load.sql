-- Upstream permissions were granted by its admins (the only writers of that
-- column), so they arrive as authority: permissions_granted is true whenever
-- upstream lists any (schemas/migrations/0014). A row saved on this node
-- after the upstream's last save is kept whole.
INSERT INTO explore.event_profile_settings AS t
    ("user", email, email_verified, email_verified_at, email_updated_at, use_local_time, notify_by_email, notify_by_browser,
     permissions, permissions_granted, created_at, updated_at)
SELECT "user", email, email_verified, email_verified_at AT TIME ZONE 'UTC',
       email_updated_at AT TIME ZONE 'UTC', use_local_time, notify_by_email, notify_by_browser,
       permissions, jsonb_array_length(permissions) > 0, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT ("user") DO UPDATE SET
    email = EXCLUDED.email,
    email_verified = EXCLUDED.email_verified,
    email_verified_at = EXCLUDED.email_verified_at,
    email_updated_at = EXCLUDED.email_updated_at,
    use_local_time = EXCLUDED.use_local_time,
    notify_by_email = EXCLUDED.notify_by_email,
    notify_by_browser = EXCLUDED.notify_by_browser,
    permissions = EXCLUDED.permissions,
    permissions_granted = EXCLUDED.permissions_granted,
    created_at = LEAST(t.created_at, EXCLUDED.created_at),
    updated_at = EXCLUDED.updated_at
WHERE t.updated_at <= EXCLUDED.updated_at;
