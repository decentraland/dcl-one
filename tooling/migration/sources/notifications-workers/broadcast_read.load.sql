INSERT INTO social.notifications
    (id, address, type, metadata, broadcast_key, occurred_at, created_at, read, read_at)
SELECT DISTINCT ON (notification_id, address)
       md5(notification_id::text || ':' || address)::uuid,
       address, type, metadata,
       'notifications-workers:' || notification_id::text,
       occurred_at, created_at, true, read_at
FROM stage
ORDER BY notification_id, address, read_at, source_address
ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO UPDATE
   SET read = true,
       read_at = CASE WHEN social.notifications.read
                      THEN social.notifications.read_at
                      ELSE EXCLUDED.read_at END
