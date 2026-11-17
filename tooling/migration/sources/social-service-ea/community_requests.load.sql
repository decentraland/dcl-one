-- Transitions first, in their own statement, so a request this batch resolves
-- frees its pair before a pending replacement in the same batch is inserted.
-- A terminal status the node holds is never reopened.
UPDATE social.community_requests t
SET type = s.type,
    status = s.status,
    created_at = s.created_at AT TIME ZONE 'UTC',
    updated_at = s.updated_at AT TIME ZONE 'UTC'
FROM stage s
WHERE t.id = s.id
  AND t.status = 'pending'
  AND t.community_id = s.community_id
  AND t.member_address = s.member_address;

-- New rows. A pending request the node already holds for the pair under
-- another id wins; none is added for a wallet the node bans there (the node
-- deletes the pair's requests when it bans).
INSERT INTO social.community_requests
    (id, community_id, member_address, type, status, created_at, updated_at)
SELECT s.id, s.community_id, s.member_address, s.type, s.status,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
WHERE NOT EXISTS (SELECT 1 FROM social.community_requests t WHERE t.id = s.id)
  AND NOT (s.status = 'pending' AND EXISTS (
      SELECT 1 FROM social.community_requests o
      WHERE o.community_id = s.community_id AND o.member_address = s.member_address
        AND o.status = 'pending'))
  AND NOT (s.status = 'pending' AND EXISTS (
      SELECT 1 FROM social.community_bans b
      WHERE b.community_id = s.community_id AND b.banned_address = s.member_address AND b.active))
ON CONFLICT (id) DO NOTHING;
