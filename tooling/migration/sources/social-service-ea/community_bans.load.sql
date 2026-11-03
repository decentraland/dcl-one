INSERT INTO social.community_bans AS t
    (community_id, banned_address, banned_by, banned_at, reason, active, unbanned_by, unbanned_at)
SELECT s.community_id, s.banned_address, s.banned_by, s.banned_at AT TIME ZONE 'UTC', s.reason, s.active,
       s.unbanned_by, s.unbanned_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
ON CONFLICT (community_id, banned_address) DO UPDATE SET
    banned_by = EXCLUDED.banned_by,
    banned_at = EXCLUDED.banned_at,
    reason = EXCLUDED.reason,
    active = EXCLUDED.active,
    unbanned_by = EXCLUDED.unbanned_by,
    unbanned_at = EXCLUDED.unbanned_at;

-- The node's invariant: an actively banned wallet has no member row. Removes
-- joins made on this node while the community was mirrored; never the owner.
DELETE FROM social.community_members m
USING stage s
WHERE s.active
  AND m.community_id = s.community_id
  AND m.member_address = s.banned_address
  AND m.role <> 'owner';
