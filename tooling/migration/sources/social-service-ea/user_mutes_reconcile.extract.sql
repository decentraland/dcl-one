-- Every pair upstream holds (the user_mutes step's filters), for deletions.
SELECT DISTINCT lower(x.muter_address) AS muter_address, lower(x.muted_address) AS muted_address
FROM user_mutes x
WHERE lower(x.muter_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(x.muted_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(x.muter_address) <> lower(x.muted_address)
