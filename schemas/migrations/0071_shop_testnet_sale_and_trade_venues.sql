ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS sepolia_trade_v1 boolean NOT NULL DEFAULT false;

-- Withdraw only the old Sepolia publication. Fencing its owner and refusing
-- an old binary's subsequent lease claim prevents publication with V1 missing.
-- The normal fenced rebuild handles projection cleanup and notification replay.
UPDATE shop.chain_cursor
SET start_block = CASE WHEN start_block = 9455328 THEN 6770056 ELSE start_block END,
    last_block = CASE WHEN last_block < start_block AND start_block = 9455328 THEN 6770055 ELSE last_block END,
    fold_through = NULL, folded_at = NULL, folded_block = NULL, owner_generation = owner_generation + 1,
    rebuild_requested_at = CASE WHEN quarantined_at IS NULL THEN timezone('UTC', now()) ELSE rebuild_requested_at END,
    sepolia_trade_v1 = true
WHERE indexer = 'trade_events' AND network = 'ethereum' AND chain_id = 11155111 AND NOT sepolia_trade_v1;

ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_sepolia_trade_v1;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_sepolia_trade_v1 CHECK (
    indexer <> 'trade_events' OR chain_id IS DISTINCT FROM 11155111 OR sepolia_trade_v1);
