-- `stage` is the whole upstream set (a `whole` step). Only local communities
-- (the communities step made every imported one local). The owner follows the
-- node's communities.owner_address, which a newer node transfer keeps.
-- dcl_migration.social_community_member_loaded holds what this load last
-- wrote: a row the node changed (role, rejoin) or removed (kick, leave, ban)
-- since stays the node's, unless upstream holds a newer join.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.social_community_member_loaded) THEN
    RAISE EXCEPTION 'social-service-ea/community_members: upstream lists no members but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE community_member_owned ON COMMIT DROP AS
SELECT s.community_id, s.member_address,
       CASE WHEN s.member_address = c.owner_address THEN 'owner'
            WHEN s.role = 'owner' THEN 'moderator'
            ELSE s.role END AS role,
       s.joined_at AT TIME ZONE 'UTC' AS joined_at
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
LEFT JOIN dcl_migration.social_community_member_loaded l
  ON l.community_id = s.community_id AND l.member_address = s.member_address
LEFT JOIN social.community_members t
  ON t.community_id = s.community_id AND t.member_address = s.member_address
WHERE l.community_id IS NULL
   OR (t.community_id IS NOT NULL AND (t.role, t.joined_at) = (l.role, l.joined_at))
   OR (t.community_id IS NULL AND l.joined_at < s.joined_at AT TIME ZONE 'UTC');

-- Keeps the one-owner index: an owner row the node holds for a wallet that no
-- longer owns the community is demoted before the new owner row lands.
UPDATE social.community_members t
SET role = 'member'
FROM social.communities c
WHERE c.id = t.community_id
  AND t.role = 'owner'
  AND t.member_address <> c.owner_address
  AND EXISTS (SELECT 1 FROM community_member_owned o
              WHERE o.community_id = t.community_id AND o.role = 'owner');

WITH written AS (
    INSERT INTO social.community_members AS t (community_id, member_address, role, joined_at)
    SELECT community_id, member_address, role, joined_at FROM community_member_owned
    ON CONFLICT (community_id, member_address) DO UPDATE SET
        role = EXCLUDED.role,
        joined_at = EXCLUDED.joined_at
    RETURNING community_id, member_address, role, joined_at
)
INSERT INTO dcl_migration.social_community_member_loaded AS l (community_id, member_address, role, joined_at)
SELECT community_id, member_address, role, joined_at FROM written
ON CONFLICT (community_id, member_address) DO UPDATE SET
    role = EXCLUDED.role,
    joined_at = EXCLUDED.joined_at;

-- Upstream removed it (kick, leave, ban): the imported row goes unless the
-- node changed it since. The owner row goes only with its community.
CREATE TEMP TABLE community_member_gone ON COMMIT DROP AS
SELECT l.community_id, l.member_address, l.role, l.joined_at
FROM dcl_migration.social_community_member_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s
                  WHERE s.community_id = l.community_id AND s.member_address = l.member_address);

DELETE FROM social.community_members m
USING community_member_gone g
WHERE m.community_id = g.community_id AND m.member_address = g.member_address
  AND m.role = g.role AND m.joined_at = g.joined_at AND m.role <> 'owner';

DELETE FROM dcl_migration.social_community_member_loaded l
USING community_member_gone g
WHERE l.community_id = g.community_id AND l.member_address = g.member_address;

DROP TABLE community_member_gone;
DROP TABLE community_member_owned;
