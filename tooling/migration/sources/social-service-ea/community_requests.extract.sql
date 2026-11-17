-- The node allows one pending request per (community, wallet); upstream does
-- not, so only the newest pending one is kept.
SELECT r.id,
       r.community_id,
       lower(r.member_address) AS member_address,
       r.type::text AS type,
       r.status::text AS status,
       r.created_at AT TIME ZONE 'UTC' AS created_at,
       r.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM community_requests r
WHERE r.type IN ('invite', 'request_to_join')
  AND r.status IN ('pending', 'accepted', 'rejected', 'cancelled')
  AND lower(r.member_address) ~ '^0x[0-9a-f]{40}$'
  AND NOT (r.status = 'pending' AND EXISTS (
      SELECT 1 FROM community_requests d
      WHERE d.community_id = r.community_id
        AND d.status = 'pending'
        AND lower(d.member_address) = lower(r.member_address)
        AND (d.created_at, d.id) > (r.created_at, r.id)))
