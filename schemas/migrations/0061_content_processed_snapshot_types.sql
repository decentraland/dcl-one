-- NULL proves all types were processed. Prior hash-only receipts lack coverage
-- information and use the empty default until replayed.
ALTER TABLE content.processed_snapshots ADD COLUMN IF NOT EXISTS entity_types text[] DEFAULT '{}';
