-- A pending request the node already holds for the pair under another id
-- wins.
INSERT INTO social.community_requests AS t
    (id, community_id, member_address, type, status, created_at, updated_at)
SELECT s.id, s.community_id, s.member_address, s.type, s.status,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
WHERE NOT (s.status = 'pending' AND EXISTS (
    SELECT 1 FROM social.community_requests o
    WHERE o.community_id = s.community_id AND o.member_address = s.member_address
      AND o.status = 'pending' AND o.id <> s.id))
ON CONFLICT (id) DO UPDATE SET
    community_id = EXCLUDED.community_id,
    member_address = EXCLUDED.member_address,
    type = EXCLUDED.type,
    status = EXCLUDED.status,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;
