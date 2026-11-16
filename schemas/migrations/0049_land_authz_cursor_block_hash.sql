-- The hash of the cursor block, checked at the start of every pass to detect a
-- reorg, and a checkpoint (an indexed block 128 to 256 blocks below the cursor
-- and its hash) a reorg rewinds to. Existing rows keep NULL; the next passes
-- record the current hash and then the checkpoint.
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS last_block_hash text;
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS checkpoint_block bigint;
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS checkpoint_hash text;
