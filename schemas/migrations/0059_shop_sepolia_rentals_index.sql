-- Sepolia shares the Ethereum supervisor and keeps its own projection cursor.
-- Its events have a separate chain-position namespace within the rental log.
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers', 'sales', 'committee', 'rentals_sepolia', 'third_party_registry'));
ALTER TABLE shop.rental_event DROP CONSTRAINT IF EXISTS rental_event_network_known;
ALTER TABLE shop.rental_event ADD CONSTRAINT rental_event_network_known CHECK (network IN ('ethereum', 'sepolia'));
