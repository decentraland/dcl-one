-- shop.nft_event records an `order_successful`'s transaction hash, which the
-- badge processor counts sales by. A nullable column without a default
-- changes the catalog only; kept rows stay NULL and are read by their chain
-- position instead.
ALTER TABLE IF EXISTS shop.nft_event ADD COLUMN IF NOT EXISTS tx_hash varchar(66);
