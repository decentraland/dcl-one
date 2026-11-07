-- Lists with their grants inlined. Timestamps are `timestamp` holding UTC.
-- is_private is what the node stores on write: no `view` grant to `*`.
-- `updated_at` (the watermark) falls back to created_at; `last_updated_at`
-- is the nullable upstream value.
SELECT l.id,
       l.name::text AS name,
       l.description::text AS description,
       lower(l.user_address) AS user_address,
       NOT EXISTS (SELECT 1 FROM favorites.acl a
                   WHERE a.list_id = l.id AND a.permission::text = 'view' AND a.grantee = '*') AS is_private,
       l.created_at AT TIME ZONE 'UTC' AS created_at,
       l.updated_at AT TIME ZONE 'UTC' AS last_updated_at,
       COALESCE(l.updated_at, l.created_at) AT TIME ZONE 'UTC' AS updated_at,
       COALESCE((SELECT jsonb_agg(DISTINCT jsonb_build_object('permission', a.permission::text,
                                                              'grantee', lower(a.grantee)))
                 FROM favorites.acl a WHERE a.list_id = l.id), '[]'::jsonb) AS grants
FROM favorites.lists l
WHERE l.id <> '70ab6873-4a03-4eb2-b331-4b8be0e0b8af'
