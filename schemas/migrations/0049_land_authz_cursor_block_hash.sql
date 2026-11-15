-- The hash of the cursor block, checked at the start of every pass to detect a
-- reorg. Existing rows keep NULL; the next pass records the current hash.
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS last_block_hash text;
