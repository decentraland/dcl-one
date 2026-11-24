CREATE TEMP TABLE incoming_slot_usage_cheque ON COMMIT DROP AS
SELECT DISTINCT ON (s.third_party_id, lower(s.salt))
       s.id, s.collection_id, s.third_party_id, lower(s.signature) AS signature, lower(s.salt) AS salt, s.qty,
       s.created_at AT TIME ZONE 'UTC' AS created_at, GREATEST(s.updated_at, s.created_at) AT TIME ZONE 'UTC' AS updated_at
FROM stage s
JOIN builder.collections c ON c.id = s.collection_id AND c.third_party_id = s.third_party_id
WHERE lower(s.salt) ~ '^0x[0-9a-f]{64}$'
  AND lower(s.signature) ~ '^0x[0-9a-f]{130}$'
  AND s.qty > 0
  AND NOT EXISTS (SELECT 1 FROM builder.slot_usage_cheques o
                  WHERE o.third_party_id = s.third_party_id AND o.salt = lower(s.salt))
ORDER BY s.third_party_id, lower(s.salt), s.created_at, s.id;

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(ARRAY(SELECT DISTINCT collection_id FROM incoming_slot_usage_cheque), '{}');
END $lock$;

WITH inserted AS (
  INSERT INTO builder.slot_usage_cheques AS q (id, collection_id, third_party_id, signature, salt, qty, created_at, updated_at)
  SELECT id, collection_id, third_party_id, signature, salt, qty, created_at, updated_at
  FROM incoming_slot_usage_cheque
  ON CONFLICT (id) DO NOTHING
  RETURNING q.id, pg_temp.builder_slot_usage_cheque_fingerprint(q) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'slot_usage_cheque', id::text, fingerprint FROM inserted;
