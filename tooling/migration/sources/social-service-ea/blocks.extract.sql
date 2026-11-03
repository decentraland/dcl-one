-- Upstream's surrogate id is dropped: the node keys a block by the pair.
SELECT lower(b.blocker_address) AS blocker_address,
       lower(b.blocked_address) AS blocked_address,
       COALESCE(b.blocked_at, 'epoch') AT TIME ZONE 'UTC' AS blocked_at
FROM blocks b
WHERE lower(b.blocker_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(b.blocked_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(b.blocker_address) <> lower(b.blocked_address)
  AND NOT EXISTS (
      SELECT 1 FROM blocks d
      WHERE lower(d.blocker_address) = lower(b.blocker_address)
        AND lower(d.blocked_address) = lower(b.blocked_address)
        AND (d.blocker_address || d.blocked_address <> lower(d.blocker_address || d.blocked_address),
             d.blocker_address, d.blocked_address)
          < (b.blocker_address || b.blocked_address <> lower(b.blocker_address || b.blocked_address),
             b.blocker_address, b.blocked_address))
