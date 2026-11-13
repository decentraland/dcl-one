-- Upstream ids kept. A (name, owner) the node already holds under another
-- id keeps the node's list; this one and its grants and picks are skipped.
-- A list the node changed later (newer updated_at) keeps its fields and grants.
INSERT INTO shop.lists AS t (id, name, description, user_address, is_private, permission, created_at, updated_at)
SELECT DISTINCT ON (s.name, s.user_address)
       s.id, s.name, s.description, s.user_address, s.is_private, NULL,
       s.created_at AT TIME ZONE 'UTC', s.last_updated_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (SELECT 1 FROM shop.lists l
                  WHERE l.name = s.name AND l.user_address = s.user_address AND l.id <> s.id)
ORDER BY s.name, s.user_address, s.created_at, s.id
ON CONFLICT (id) DO UPDATE
   SET name = EXCLUDED.name, description = EXCLUDED.description,
       user_address = EXCLUDED.user_address, is_private = EXCLUDED.is_private,
       created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at
 WHERE COALESCE(t.updated_at, t.created_at) <= COALESCE(EXCLUDED.updated_at, EXCLUDED.created_at);

DO $$
DECLARE skipped bigint;
BEGIN
    SELECT count(*) INTO skipped FROM stage s WHERE NOT EXISTS (SELECT 1 FROM shop.lists l WHERE l.id = s.id);
    IF skipped > 0 THEN
        RAISE NOTICE 'marketplace-favorites-server/lists: % list(s) skipped: the owner already holds a list of that name; their grants and picks are not imported', skipped;
    END IF;
END $$;

-- The landed lists' grants become the source's.
DELETE FROM shop.list_acl a
USING stage s
JOIN shop.lists l ON l.id = s.id AND l.user_address = s.user_address
WHERE a.list_id = s.id
  AND COALESCE(l.updated_at, l.created_at) <= COALESCE(s.last_updated_at, s.created_at) AT TIME ZONE 'UTC'
  AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(s.grants) g
                  WHERE g ->> 'permission' = a.permission AND g ->> 'grantee' = a.grantee);

INSERT INTO shop.list_acl (list_id, permission, grantee)
SELECT s.id, g ->> 'permission', g ->> 'grantee'
FROM stage s
JOIN shop.lists l ON l.id = s.id AND l.user_address = s.user_address
CROSS JOIN LATERAL jsonb_array_elements(s.grants) g
WHERE COALESCE(l.updated_at, l.created_at) <= COALESCE(s.last_updated_at, s.created_at) AT TIME ZONE 'UTC'
ON CONFLICT DO NOTHING;
