-- The missing-objects step creates shop.third_party_registry_event and its index.
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known
    CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers', 'sales', 'committee', 'rentals_sepolia', 'third_party_registry'));
