-- The missing-objects step creates shop.collection_event, shop.collection_hint,
-- shop.collection_seed and their indexes.
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known
    CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers', 'sales', 'committee', 'rentals_sepolia', 'third_party_registry', 'collections'));
