-- The `sales` chain index (src/shop/market/sales_index.rs) keeps its cursor
-- in shop.chain_cursor beside `nft`, `trade_events`, `rentals` and
-- `mana_transfers`; schemas/30_shop.sql admits the indexer name in the
-- CHECK, and a database stamped before it re-declares the constraint here.
-- Its tables are created and seeded by `schema migrate`'s missing-objects
-- step: shop.sale_event (the kept logs: store mints and ERC721BidV2 bids on
-- Polygon, append-only with the cursor advance, keyed by chain position),
-- shop.nft_bid (one row per bidder and token, the fold of the bid rows,
-- status open, accepted or cancelled) and shop.sale_venue (the four Polygon
-- venues with their pinned royalties cut).
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers', 'sales'));
