ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS sepolia_trade_v1 boolean NOT NULL DEFAULT false;

-- Withdraw only the old Sepolia publication. Fencing its owner and refusing
-- an old binary's subsequent lease claim prevents publication with V1 missing.
-- The normal fenced rebuild handles projection cleanup and notification replay.
-- A quarantined cursor keeps its start and missing-V1 marker until revalidation
-- clears its chain binding and the new writer proves the stored checkpoint.
-- Old lease claims still propose an unquarantined row and fail the check.
UPDATE shop.chain_cursor
SET start_block = CASE WHEN start_block = 9455328 THEN 6770056 ELSE start_block END,
    last_block = CASE WHEN last_block < start_block AND start_block = 9455328 THEN 6770055 ELSE last_block END,
    fold_through = NULL, folded_at = NULL, folded_block = NULL, owner_generation = owner_generation + 1,
    rebuild_requested_at = timezone('UTC', now()),
    sepolia_trade_v1 = true
WHERE indexer = 'trade_events' AND network = 'ethereum' AND chain_id = 11155111
  AND NOT sepolia_trade_v1 AND quarantined_at IS NULL;

ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_sepolia_trade_v1;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_sepolia_trade_v1 CHECK (
    indexer <> 'trade_events' OR chain_id IS DISTINCT FROM 11155111 OR sepolia_trade_v1 OR quarantined_at IS NOT NULL);
