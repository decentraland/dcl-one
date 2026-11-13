-- shop.nft_bid admits the `invalidated` status (src/shop/market/sales_index.rs):
-- an open bid on a token whose other bid was accepted after it, which the
-- ERC721BidV2 contract drops without an event. schemas/30_shop.sql declares
-- it in the CHECK; a database that created the table before it re-declares
-- the constraint here (IF EXISTS: a database stamped before the table gains
-- it from the missing-objects pass with the CHECK as declared).
ALTER TABLE IF EXISTS shop.nft_bid DROP CONSTRAINT IF EXISTS nft_bid_status_known;
ALTER TABLE IF EXISTS shop.nft_bid ADD CONSTRAINT nft_bid_status_known CHECK (status IN ('open', 'accepted', 'cancelled', 'invalidated'));
