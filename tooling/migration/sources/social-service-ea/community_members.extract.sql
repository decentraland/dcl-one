-- The node proves every community authority against these rows, so the
-- owner row follows communities.owner_address: that wallet is 'owner'
-- (synthesized from the community when upstream has no row for it), any
-- other 'owner' row becomes 'moderator'. 'none' rows and actively banned
-- wallets (other than the owner) are not members; of case variants the
-- lowercase spelling wins. Role changes move no upstream timestamp: full
-- re-scan.
SELECT m.community_id,
       lower(m.member_address) AS member_address,
       CASE
           WHEN lower(m.member_address) = lower(c.owner_address) THEN 'owner'
           WHEN m.role = 'owner' THEN 'moderator'
           ELSE m.role::text
       END AS role,
       m.joined_at AT TIME ZONE 'UTC' AS joined_at
FROM community_members m
JOIN communities c ON c.id = m.community_id
WHERE m.role IN ('owner', 'moderator', 'member')
  AND lower(m.member_address) ~ '^0x[0-9a-f]{40}$'
  AND (lower(m.member_address) = lower(c.owner_address)
       OR NOT EXISTS (
           SELECT 1 FROM community_bans b
           WHERE b.community_id = m.community_id AND b.active
             AND lower(b.banned_address) = lower(m.member_address)))
  AND NOT EXISTS (
      SELECT 1 FROM community_members d
      WHERE d.community_id = m.community_id
        AND d.role IN ('owner', 'moderator', 'member')
        AND lower(d.member_address) = lower(m.member_address)
        AND (d.member_address <> lower(d.member_address), d.member_address)
          < (m.member_address <> lower(m.member_address), m.member_address))
UNION ALL
SELECT c.id AS community_id,
       lower(c.owner_address) AS member_address,
       'owner' AS role,
       c.created_at AT TIME ZONE 'UTC' AS joined_at
FROM communities c
WHERE lower(c.owner_address) ~ '^0x[0-9a-f]{40}$'
  AND NOT EXISTS (
      SELECT 1 FROM community_members m
      WHERE m.community_id = c.id
        AND m.role IN ('owner', 'moderator', 'member')
        AND lower(m.member_address) = lower(c.owner_address))
