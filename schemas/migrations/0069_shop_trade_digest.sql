-- The EIP-712 digest a V3 trade was signed over, stored at creation or copied
-- from its source, so a V3 cancellation applies even when the fold cannot
-- rebuild the digest from the row; and the mark of a V3 trade whose digest
-- the fold could not establish, which the open-listing reads leave out.
-- Existing rows keep NULL in both: the fold retries every V3 trade without a
-- digest on its next pass. No backfill and no rewrite.
ALTER TABLE shop.trades ADD COLUMN IF NOT EXISTS trade_digest text;
ALTER TABLE shop.trades ADD CONSTRAINT trades_trade_digest_hex
    CHECK (trade_digest ~ '^0x[0-9a-f]{64}$');
ALTER TABLE shop.trade_chain_state ADD COLUMN IF NOT EXISTS digest_failed_at timestamp without time zone;
ALTER TABLE shop.trade_chain_state ADD CONSTRAINT trade_chain_state_digest_or_failure
    CHECK (digest IS NULL OR digest_failed_at IS NULL);
