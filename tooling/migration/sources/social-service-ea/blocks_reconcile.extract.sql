-- Every pair upstream holds (the blocks step's filters), for deletions.
SELECT DISTINCT lower(x.blocker_address) AS blocker_address, lower(x.blocked_address) AS blocked_address
FROM blocks x
WHERE lower(x.blocker_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(x.blocked_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(x.blocker_address) <> lower(x.blocked_address)
