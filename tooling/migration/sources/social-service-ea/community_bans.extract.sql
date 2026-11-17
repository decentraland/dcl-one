-- Upstream re-bans set active = true and keep the old unbanned_by/at; rows
-- from before the unban columns existed are inactive without them. The node
-- requires the lift fields exactly when the ban is lifted.
SELECT b.community_id,
       lower(b.banned_address) AS banned_address,
       lower(b.banned_by) AS banned_by,
       b.banned_at AT TIME ZONE 'UTC' AS banned_at,
       b.reason,
       b.active,
       CASE WHEN b.active THEN NULL ELSE lower(COALESCE(b.unbanned_by, b.banned_by)) END AS unbanned_by,
       CASE WHEN b.active THEN NULL ELSE COALESCE(b.unbanned_at, b.banned_at) AT TIME ZONE 'UTC' END AS unbanned_at
FROM community_bans b
WHERE lower(b.banned_address) ~ '^0x[0-9a-f]{40}$'
  AND NOT EXISTS (
      SELECT 1 FROM community_bans d
      WHERE d.community_id = b.community_id
        AND lower(d.banned_address) = lower(b.banned_address)
        AND (d.banned_address <> lower(d.banned_address), d.banned_address)
          < (b.banned_address <> lower(b.banned_address), b.banned_address))
