ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_indexer_known;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events', 'rentals', 'mana_transfers'));
