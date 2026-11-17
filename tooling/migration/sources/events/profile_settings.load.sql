-- 0014 revokes permissions the node's own profile endpoint let users write;
-- upstream's column is written only by its admins, so a non-empty list
-- arrives granted. Imported grants are counted for operator review. A row
-- saved on this node after upstream's last save is kept whole.
DO $grants$ DECLARE n bigint; names text; BEGIN
  SELECT count(*), string_agg(DISTINCT p, ', ' ORDER BY p) INTO n, names
  FROM stage s CROSS JOIN LATERAL jsonb_array_elements_text(s.permissions) p
  WHERE jsonb_array_length(s.permissions) > 0;
  IF n > 0 THEN RAISE NOTICE 'events/profile_settings: importing % permission grants (%); review them before cutover', n, names;
  ELSE NULL; END IF; END $grants$;

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
