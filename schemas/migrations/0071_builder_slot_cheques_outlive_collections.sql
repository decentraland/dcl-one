ALTER TABLE IF EXISTS builder.slot_usage_cheques ALTER COLUMN collection_id DROP NOT NULL;
ALTER TABLE IF EXISTS builder.slot_usage_cheques DROP CONSTRAINT IF EXISTS slot_usage_cheques_collection_id_fkey;
ALTER TABLE IF EXISTS builder.slot_usage_cheques ADD CONSTRAINT slot_usage_cheques_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES builder.collections (id) ON DELETE SET NULL;
