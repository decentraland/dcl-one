-- social_settings has no timestamp: full re-scan. Case variants of one wallet
-- collapse to the lowercase spelling (what upstream writes
-- today), else the lexically smallest.
SELECT lower(s.address) AS address,
       s.private_messages_privacy::text AS private_messages_privacy,
       s.blocked_users_messages_visibility::text AS blocked_users_messages_visibility,
       s.show_situation_reactions::text AS show_situation_reactions
FROM social_settings s
WHERE lower(s.address) ~ '^0x[0-9a-f]{40}$'
  AND NOT EXISTS (
      SELECT 1 FROM social_settings d
      WHERE lower(d.address) = lower(s.address) AND (d.address <> lower(d.address), d.address) < (s.address <> lower(s.address), s.address))
