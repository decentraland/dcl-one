-- Only into local communities (the previous step made every imported one
-- local). A previous owner row the node holds is demoted first so the
-- one-owner index holds when ownership moved upstream between runs.
UPDATE social.community_members t
SET role = 'member'
FROM stage s
WHERE s.role = 'owner'
  AND t.community_id = s.community_id
  AND t.role = 'owner'
  AND t.member_address <> s.member_address;

INSERT INTO social.community_members AS t (community_id, member_address, role, joined_at)
SELECT s.community_id, s.member_address, s.role, s.joined_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
ON CONFLICT (community_id, member_address) DO UPDATE SET
    role = EXCLUDED.role,
    joined_at = EXCLUDED.joined_at;
