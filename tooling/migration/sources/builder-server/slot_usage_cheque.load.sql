INSERT INTO builder.slot_usage_cheques AS k
    (id, collection_id, third_party_id, signature, salt, qty, created_at, updated_at)
SELECT DISTINCT ON (s.third_party_id, lower(s.salt))
       s.id, s.collection_id, s.third_party_id, lower(s.signature), lower(s.salt), s.qty,
       s.created_at AT TIME ZONE 'UTC', GREATEST(s.updated_at, s.created_at) AT TIME ZONE 'UTC'
FROM stage s
JOIN builder.collections c ON c.id = s.collection_id AND c.third_party_id = s.third_party_id
WHERE lower(s.salt) ~ '^0x[0-9a-f]{64}$'
  AND lower(s.signature) ~ '^0x[0-9a-f]{130}$'
  AND s.qty > 0
  AND NOT EXISTS (SELECT 1 FROM builder.slot_usage_cheques o
                  WHERE o.third_party_id = s.third_party_id AND o.salt = lower(s.salt) AND o.id <> s.id)
ORDER BY s.third_party_id, lower(s.salt), s.created_at, s.id
ON CONFLICT (id) DO NOTHING;
