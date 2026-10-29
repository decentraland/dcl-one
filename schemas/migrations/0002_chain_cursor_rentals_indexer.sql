-- The `rentals` chain index (SHOP.md slice 3, src/shop/rentals/index.rs)
-- keeps its cursor in shop.chain_cursor beside `nft` and `trade_events`;
-- schema/30_shop.sql admits the new indexer name in the CHECK, and a
-- database stamped before it re-declares the constraint here. Its own table,
-- shop.rental_event, is created by `schema migrate`'s missing-objects step.
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events', 'rentals'));
