-- A legacy NFT projection rebuild can reach the same block and hash while
-- omitting CreateEstate logs. Invalidate the archive proof only for that reset.
CREATE OR REPLACE FUNCTION shop.reset_estate_creation_repair() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM shop.worker_progress WHERE worker = 'nft_estate_creations' AND scope = 1;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE TRIGGER reset_estate_creation_repair
AFTER UPDATE OF last_block ON shop.chain_cursor
FOR EACH ROW WHEN (OLD.indexer = 'nft' AND OLD.network = 'ethereum'
    AND NEW.last_block < OLD.last_block AND NEW.last_block = NEW.start_block - 1
    AND NEW.last_block_hash IS NULL AND NEW.reorg_rewinds = OLD.reorg_rewinds
    AND OLD.rebuild_requested_at IS NOT NULL AND NEW.rebuild_requested_at IS NULL)
EXECUTE FUNCTION shop.reset_estate_creation_repair();
