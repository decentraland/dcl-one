-- An item copied from a graph before its network was proven cannot enrich
-- a testnet NFT. Keep the descriptive copy available during its first
-- row-verifying pass. A writer that cannot mark the rows cannot publish it.
ALTER TABLE shop.items ADD COLUMN IF NOT EXISTS testnet_verified boolean NOT NULL DEFAULT false;
ALTER TABLE shop.mirror_state ADD COLUMN IF NOT EXISTS marks_item_rows boolean NOT NULL DEFAULT false;
UPDATE shop.mirror_state SET chain_id=NULL, pass_block_hash=NULL,
    pass_started_at=NULL, cursor_updated_at=NULL, cursor_id=NULL
WHERE mirror='items' AND NOT marks_item_rows AND
    (chain_id IN (11155111,80002) OR pass_block_hash IS NOT NULL);
ALTER TABLE shop.mirror_state DROP CONSTRAINT IF EXISTS mirror_state_verified_item_rows;
ALTER TABLE shop.mirror_state ADD CONSTRAINT mirror_state_verified_item_rows CHECK (
    mirror <> 'items' OR marks_item_rows OR
    (pass_block_hash IS NULL AND COALESCE(chain_id,0) NOT IN (11155111,80002))
);
