SELECT lower(m.muter_address) AS muter_address,
       lower(m.muted_address) AS muted_address,
       m.muted_at AT TIME ZONE 'UTC' AS muted_at
FROM user_mutes m
WHERE lower(m.muter_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(m.muted_address) ~ '^0x[0-9a-f]{40}$'
  AND lower(m.muter_address) <> lower(m.muted_address)
  AND NOT EXISTS (
      SELECT 1 FROM user_mutes d
      WHERE lower(d.muter_address) = lower(m.muter_address)
        AND lower(d.muted_address) = lower(m.muted_address)
        AND (d.muter_address || d.muted_address <> lower(d.muter_address || d.muted_address),
             d.muter_address, d.muted_address)
          < (m.muter_address || m.muted_address <> lower(m.muter_address || m.muted_address),
             m.muter_address, m.muted_address))
