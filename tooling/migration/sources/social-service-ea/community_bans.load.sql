-- A ban the node lifted or re-applied since this source last wrote it (its
-- state no longer matches dcl_migration.social_community_ban_loaded) stays
-- the node's.
WITH owned AS (
    SELECT s.*
    FROM stage s
    JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
    LEFT JOIN dcl_migration.social_community_ban_loaded l
      ON l.community_id = s.community_id AND l.banned_address = s.banned_address
    LEFT JOIN social.community_bans t
      ON t.community_id = s.community_id AND t.banned_address = s.banned_address
    WHERE l.community_id IS NULL OR t.community_id IS NULL
       OR (t.active, t.unbanned_at) IS NOT DISTINCT FROM (l.active, l.unbanned_at)
), written AS (
    INSERT INTO social.community_bans AS t
        (community_id, banned_address, banned_by, banned_at, reason, active, unbanned_by, unbanned_at)
    SELECT o.community_id, o.banned_address, o.banned_by, o.banned_at AT TIME ZONE 'UTC', o.reason, o.active,
           o.unbanned_by, o.unbanned_at AT TIME ZONE 'UTC'
    FROM owned o
    ON CONFLICT (community_id, banned_address) DO UPDATE SET
        banned_by = EXCLUDED.banned_by,
        banned_at = EXCLUDED.banned_at,
        reason = EXCLUDED.reason,
        active = EXCLUDED.active,
        unbanned_by = EXCLUDED.unbanned_by,
        unbanned_at = EXCLUDED.unbanned_at
    RETURNING community_id, banned_address, active, unbanned_at
)
INSERT INTO dcl_migration.social_community_ban_loaded AS l (community_id, banned_address, active, unbanned_at)
SELECT community_id, banned_address, active, unbanned_at FROM written
ON CONFLICT (community_id, banned_address) DO UPDATE SET
    active = EXCLUDED.active,
    unbanned_at = EXCLUDED.unbanned_at;

-- The node's invariant: an actively banned wallet has no member row. Only
-- where the ban is in force on the node (a node unban keeps a rejoined
-- member); never the owner.
DELETE FROM social.community_members m
USING stage s
JOIN social.community_bans b
  ON b.community_id = s.community_id AND b.banned_address = s.banned_address AND b.active
WHERE s.active
  AND m.community_id = s.community_id
  AND m.member_address = s.banned_address
  AND m.role <> 'owner';
