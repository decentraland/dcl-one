-- The missing-objects step creates shop.collection_projection,
-- shop.collection_walk and collection_event_replay_idx.
ALTER TABLE shop.items ADD COLUMN IF NOT EXISTS minters text[] NOT NULL DEFAULT '{}';
ALTER TABLE shop.items ADD COLUMN IF NOT EXISTS minter_allowances numeric(78,0)[] NOT NULL DEFAULT '{}';
ALTER TABLE IF EXISTS shop.collection_hint ADD COLUMN IF NOT EXISTS read_block bigint;
