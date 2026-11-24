-- A market chain index cursor whose block was found on another branch than
-- its stored hash (services/catalyst/shop/src/market/chain_walk.rs): its
-- publication is withdrawn and diverged_at set while its recovery searches;
-- the rewind, or a pass that proves the cursor again, clears it. Readers
-- that use the cursor's height without its publication (the sales fold's
-- seller inference, the tips' cross-sender eviction) skip it meanwhile.
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS diverged_at timestamp without time zone;
