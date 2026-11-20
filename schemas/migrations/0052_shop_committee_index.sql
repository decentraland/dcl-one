-- The `committee` chain index (services/catalyst/shop/src/market/committee_index.rs) keeps its
-- cursor in shop.chain_cursor beside the other marketplace indexes;
-- schemas/30_shop.sql admits the indexer name in the CHECK, and a database
-- stamped before it re-declares the constraint here. Its table,
-- shop.committee_event, is created by `schema migrate`'s missing-objects step.
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers', 'sales', 'committee'));
